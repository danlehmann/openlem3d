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

### Level music (verified)

**Method.** DOSBox-X logs CD-audio playback when its `[log]` section sets
`misc = debug` (`tools/original/launch.ps1 -ExtraConf` loads such a file).
Each play request then shows up as `CDROM: Playing track # N` in the log
file; track numbers count the data track as 1, so audio tracks are 2–24. No
debugger is involved; this is the emulator's own device log. Levels beyond
the first of each rating were reached with level codes (see
[level.md](level.md#level-codes)), on the real, unpatched image.

**When music plays (verified).** The track starts when the level starts
(after "Continue" on the briefing), not on the briefing or in the menus.
Starting the same level again always gave the same track: Fun 1 about 17
times in one session, Practice "Turner" twice in separate sessions.

**The track depends only on theme (`0x143`) and music index (`0x151`).**
All 47 levels observed (40 rated levels, 36 of them reached by code, and 7
Practice levels) fit one table, and every (theme, music) pair used by the
100 levels was observed at least once; where a pair was observed on several
levels, they always played the same track. Music indices 0 and 2 always play
the same track; index 1 plays a different one. No level played track 2, 3 or
24.

| Theme (`0x143`) | Music 0 and 2 | Music 1 |
|---|---|---|
| 1 | 13 | 22 |
| 2 | 8 | 19 |
| 3 | 4 | 15 |
| 4 | 14 | 23 |
| 5 | 10 | 21 |
| 6 | 7 | 18 |
| 7 | 5 | 16 |
| 8 | 9 | 20 |
| 9 | 6 | 17 |
| 10 | 11 | 12 |

**Editions differ.** Our CD is the "3D Lemmings" edition (the title on its
main menu); its Fun list starts "Take A Dive", "That's Right", "The Bean
Machine" (verified), so file `LEVEL.nnn` is level n + 1. An online
"Lemmings 3D" version the owner played starts Fun with "Candyland Climber"
(`LEVEL.006` here) and shows that level with other textures (multicoloured
blocks, a chocolate floor), so its level data differ (owner's observation).
Everything in these specs is for the "3D Lemmings" CD.

Observations (one log line per level start):

| Level | File | Theme | Music | Track |
|---|---|---|---|---|
| Fun 1 "TAKE A DIVE" | `LEVEL.000` | 7 | 0 | 5 |
| Fun 2 "THAT'S RIGHT" | `LEVEL.001` | 10 | 1 | 12 |
| Fun 3 "THE BEAN MACHINE" | `LEVEL.002` | 4 | 2 | 14 |
| Fun 4 "IT'S A RUN AROUND" | `LEVEL.003` | 1 | 0 | 13 |
| Fun 5 "BOUNCE BOUNCE" | `LEVEL.004` | 9 | 0 | 6 |
| Fun 10 "CASTLE LEMMALOT" | `LEVEL.009` | 1 | 1 | 22 |
| Fun 12 "ALPINE ASSAULT COURSE" | `LEVEL.011` | 7 | 1 | 16 |
| Fun 14 "SLIPPERY MAZE" | `LEVEL.013` | 7 | 0 | 5 |
| Fun 17 "HOLE IN TEN" | `LEVEL.016` | 5 | 0 | 10 |
| Fun 20 "ALILEMM'S" | `LEVEL.019` | 3 | 0 | 4 |
| Tricky 1 "JELLY CLIMBER" | `LEVEL.020` | 4 | 1 | 23 |
| Tricky 2 "WHICH EXIT ?" | `LEVEL.021` | 9 | 1 | 17 |
| Tricky 4 "FORE!" | `LEVEL.023` | 5 | 1 | 21 |
| Tricky 5 "BREAKOUT" | `LEVEL.024` | 6 | 1 | 18 |
| Tricky 7 "FOLLOW THE YELLOW BRICK ROAD" | `LEVEL.026` | 10 | 2 | 11 |
| Tricky 10 "TOOTEN LEMMING" | `LEVEL.029` | 2 | 0 | 8 |
| Tricky 12 "DOT TO DOT" | `LEVEL.031` | 3 | 1 | 15 |
| Tricky 15 "CHOCOLATE DROP" | `LEVEL.034` | 4 | 1 | 23 |
| Tricky 17 "GARDEN MAZE" | `LEVEL.036` | 8 | 1 | 20 |
| Tricky 18 "PLAY TIME" | `LEVEL.037` | 10 | 1 | 12 |
| Tricky 20 "KING CODER'S TOMB" | `LEVEL.039` | 2 | 1 | 19 |
| Taxing 1 "SPAGHETTI JUNCTION" | `LEVEL.040` | 8 | 2 | 9 |
| Taxing 2 "PICKY PLATFORM" | `LEVEL.041` | 9 | 2 | 6 |
| Taxing 5 "3D - A LEMMING ODYSSEY" | `LEVEL.044` | 3 | 0 | 4 |
| Taxing 7 "IF THE TIMING IS RIGHT!" | `LEVEL.046` | 1 | 1 | 22 |
| Taxing 11 "CHAOS MAZE" | `LEVEL.050` | 8 | 2 | 9 |
| Taxing 14 "AWAY TEAM" | `LEVEL.053` | 10 | 0 | 11 |
| Taxing 16 "THE ARENA" | `LEVEL.055` | 2 | 0 | 8 |
| Taxing 19 "LEMMTRIS" | `LEVEL.058` | 6 | 0 | 7 |
| Taxing 20 "DEATH SLIDE" | `LEVEL.059` | 7 | 2 | 5 |
| Mayhem 1 "THE FIVE ARCHES" | `LEVEL.060` | 1 | 2 | 13 |
| Mayhem 3 "TOWER OF LEMLAB" | `LEVEL.062` | 3 | 2 | 4 |
| Mayhem 5 "THE PRISONER" | `LEVEL.064` | 2 | 2 | 8 |
| Mayhem 6 "FAMILY TREE" | `LEVEL.065` | 6 | 2 | 7 |
| Mayhem 9 "OVER THE TOP" | `LEVEL.068` | 7 | 2 | 5 |
| Mayhem 12 "RAIDERS OF THE LOST LEMMING" | `LEVEL.071` | 2 | 1 | 19 |
| Mayhem 14 "JELLY BELLY ISLANDS" | `LEVEL.073` | 4 | 0 | 14 |
| Mayhem 15 "HOLE IN ONE, TWO, THREE!" | `LEVEL.074` | 5 | 2 | 10 |
| Mayhem 18 "CASTLE PERALUS" | `LEVEL.077` | 1 | 2 | 13 |
| Mayhem 20 "FINAL MAZE" | `LEVEL.079` | 8 | 0 | 9 |
| Practice "Blocker" | `LEVEL.080` | 8 | 0 | 9 |
| Practice "Turner" | `LEVEL.081` | 8 | 1 | 20 |
| Practice "Bomber" | `LEVEL.082` | 8 | 2 | 9 |
| Practice "Builder" | `LEVEL.083` | 8 | 0 | 9 |
| Practice "Basher" | `LEVEL.084` | 8 | 1 | 20 |
| Practice "Floater" | `LEVEL.088` | 8 | 2 | 9 |
| Practice "Trampoline" | `LEVEL.098` | 8 | 0 | 9 |

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
| `GFX/BGRD.000` | 1 | 15,360 bytes. |
| `GFX/OVERLAY.000` | 1 | 12,288 bytes: 24 one-bit 64×64 frames, 8 bytes per row, the most significant bit leftmost. Frames 0–15 are a crack network growing frame by frame (each a superset of the last), painted black over the faces of a block being bashed or dug; frames 16–23 are irregular blobs, not seen in use (verified against the Practice "Basher" demo: stage 15 matched frame 15 row for row). |
| `GFX/LM3D.PAL` | 1 | Main palette. See [graphics.md](graphics.md). |
| `GFX/SCENE000`–`010.{RNC,SVG,PAL,SVP}` | 11 sets | Theme preview screens. |
| `GFX/INTRO1`–`7.{RNC,SVG}` | 7 sets | Intro screens. |
| `GFX/*.RNC` (`ICONS`, `MOUSE`, `TITLE`, `LOADING`, …) | | UI graphics. |
| `GFX/*.GFX`, `GFX/*.FNT`, `GFX/TITLE.MHC` | | UI graphics and fonts. |
| `LEMM/LEMM.MHC`, `LEMM/LEMM128.MHC` | 2 | Lemming sprites, 64 and 128 px. |
| `ANIM/*` | 11 | Large files, probably FMV cutscenes. |
| `BMPS/800BMPS/*.BMP`, `BMPS/1024BMPS/*.BMP` | 10 + 10 | Per-theme backdrops for high-resolution modes. |
| `SOUND/**` | 331 | Sound effects and sound-card music, several driver variants (e.g. `SOUND/AWE/`). |
| `REPLAYS/REP.nnn`, `REPLAYS/REPLAY.nnn` | 24 | RNC-compressed demo replays, mostly 64,802 bytes unpacked. `REPLAY.080`–`099` are the Practice levels' demos (their briefings offer "Enter = Demo"). Unpacked, `REPLAY.080` starts with the 16-bit value 80, then 10,800 records of 6 bytes holding small signed values (01, 02, FF, FE): apparently recorded input, such as mouse movement and buttons, about 36 records per second over the 5-minute level (inferred, not decoded). |
