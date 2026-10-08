# Sound effects (`SOUND/`)

## Sample files `SOUND/SPOTFX/*.U8`, `SOUND/VOXFX/*.U8` (verified)

`SPOTFX` holds effects (bricks, splash, explosion, traps) and `VOXFX` the
lemmings' voices ("Let's go", "Oh no", "Yippee", the skill names). Each
file is a 32-byte header followed by mono 8-bit samples.

| Offset | Size | Field |
|---|---|---|
| `0x00` | 4 | Offset of the sample data: always 32 |
| `0x04` | 4 | Offset of the last data byte (file size − 1) |
| `0x08` | 4 | Loop start offset, 0 for none |
| `0x0C` | 4 | Loop end offset (last byte of the loop) |
| `0x10` | 2 | Sample rate in Hz: 22050 for most, 11025 or 11000 for some |
| `0x12` | 2 | `06 01` in every file; meaning unknown |
| `0x14` | 12 | Zero |

**The samples are signed** despite the `.U8` name: silence is `00`, with
small values either side (`01`, `FF`). Verified: all 81 files parse with
`l3d_formats::sound::Sample`, and the lengths at the stated rates match
the words spoken (for example "Let's go" 0.63 s, "Yippee" 0.53 s).
`BLANK1.U8` has no data. Three files loop over their whole length: the
end-of-level `CHEERS1` and `BOOS`, and the password screen's `SNAREL2B`.

## When each sample plays (`SOUND/SAMPLIST.TXT`)

The disc's own sample list says when each sample is used; its comments are
the source for these (unverified in play):

| Sample | Event |
|---|---|
| `VOXFX/BLOCKER1` … `FLOATER1` | The matching skill is selected |
| `VOXFX/PRACTISE`, `FUN2`, `TRICKY1`, `TAXING1`, `MAYHEM1` | A rating is selected |
| `VOXFX/LETSGO1` | Start of a level |
| `VOXFX/OK2`, `UH_UH1` | A lemming takes a skill, or can't |
| `VOXFX/OHNO1` | About to blow up |
| `VOXFX/DROWN1`, `SPOTFX/SPLASH` | A lemming drowns / falls into water |
| `VOXFX/WOUNDED1` | A lemming splats |
| `SPOTFX/EXPLOSN3` | A lemming blows up |
| `VOXFX/YIPPEE1` | A lemming reaches the exit |
| `SPOTFX/FANFARE` | Enough lemmings have exited |
| `VOXFX/CHEERS1`, `BOOS` | End of level, passed or failed |
| `VOXFX/SHRUG1` | A builder has finished |
| `SPOTFX/BRICKS1`, `GRAVEL3`, `PICKAXE4`, `HAMMER1` | Building (no comment), digging, mining, bashing (ours: each brick, dig and mine stroke, and each 8-tick basher swing while something is in front of it; provisional) |
| `SPOTFX/ANVIL1` | A digger, miner or basher hits metal |
| `SPOTFX/SUCKER2` | Climbing |
| `SPOTFX/BROLLY1` | An umbrella opens |
| `SPOTFX/BOING1`, `CTAPULT1`, `TELEPORT` | Trampoline, catapult, teleporter |
| `SPOTFX/FLAMETHR`, `MANTRAP1`, `CRACKLE2` | Flame-blower, man trap, laser trap |
| `SPOTFX/LIGHTNG` | A lemming leaves the level (lightning) |
| `SPOTFX/TICK1` | Time running out |
| `SPOTFX/SCALE` | Lemming view or highlight beep |
| `VOXFX/GEDDON1` | Armageddon (nuke) clicked |
| `VOXFX/PLAY`, `CODE`, `OPTIONS`, `EXIT` | The title screen's buttons |
| `VOXFX/BYEBYE1` | Leaving the game |
| `VOXFX/HUP3`, `SPOTFX/SNAREL2B` | A letter typed on the password screen; its background |
| `SPOTFX/POSITIVE`, `NEGATIVE` | Password right or wrong |
| `VOXFX/CAMRAONE`, `CAMRATWO`, `CAMRA3`, `CAMRA4` | Camera 1–4 chosen |
| `VOXFX/HILITE`, `VIRTUAL1`, `DEFLECT`, `MUD1`, `ONEWAY`, `SPLITTER`, `SLIPPER1`, `ROCKSL1`, `SPRING`, `TRAMPOL1`, `TELEPORT` | Practice-menu icons |
| `SPOTFX/LOGO2` | The swish at the start of the intro |

The list ends the "4 MB" set after `GEDDON1`; the rest are for machines
with 8 MB. Some entries have no comment (`AAH2`, `FALLER3`, `CREAKY2`,
`TONE1`); others name uses not listed above, such as `THUD1` ("stunned
lemming"), `FLYER1` ("lemming flying past camera") and `SHOTGUN1` /
`RELOAD`.

The `SOUND/AWE`, `GM`, `GUS`, `LAPC`, `SBL` and `SCC1` folders hold
per-sound-card `.SND` files named after the themes and ratings, presumably
the music for each card (unverified); we play the CD audio tracks instead
(see [disc.md](disc.md)).