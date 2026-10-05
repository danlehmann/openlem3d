# Running and driving the original game

This document covers running the original *Lemmings 3D* in DOSBox-X and
driving it from scripts, so agents can make side-by-side comparisons. Read
`docs/GROUNDRULES.md` first. In particular:

- We only observe the game as a player would. Do not use the DOSBox-X debugger
  in any form, including code stepping, disassembly views and its debugger MCP
  server (`mcp_server`, kept at `0` in `lem3d.conf`).
- Screenshots of the original game never go into the repo. `shot.ps1` refuses
  to write inside the repo, except under the git-ignored `temp/`.

Status tags used below: **[verified]** means checked in the running game with
this harness. **[unverified]** means it comes from public sources or guesswork
and has not been confirmed yet.

## 1. Setup (one-time, per machine)

| What | Default location | Override (env var) |
|---|---|---|
| DOSBox-X **full** build | `temp/dosbox-full/bin/ARM64/Release/dosbox-x.exe` | `LEM3D_DOSBOX` |
| CD image (`.cue` + `.bin`) | the `.cue` in `$OPENLEM3D_DATA`, else `gamedata/` | `LEM3D_CUE` |
| DOS drive C: | `temp/dosbox_c/` | `LEM3D_CDRIVE` |
| Screenshot folder (outside the repo, or under the git-ignored `temp/`) | `temp/shots` | `LEM3D_SHOTS` |
| State file (DOSBox-X process id) | `%TEMP%\openlem3d-dosbox-state.json` | `LEM3D_STATE` |

**Use a full DOSBox-X build, not an "osfree" one.** The build first placed in
`temp/dosbox/` is an *OS-free* release, which has no built-in DOS (no `MOUNT`,
no `IMGMOUNT`, no shell), so the game cannot run on it. We use the regular
portable build, `dosbox-x-vsbuild-arm64-2026.10.01-portable.zip`, from the
DOSBox-X GitHub releases (tag `dosbox-x-v2026.10.01`), unpacked to
`temp/dosbox-full/`. Use its SDL1 `Release` build.

**The game needs no installation.** It runs straight from the CD as
`D:\L3D.EXE`. The CD's `SETUP.EXE` only writes a small settings file,
`C:\LM3D.CD\LM3D.CFG`, which the game needs. To create it on a fresh machine:

```powershell
tools/original/launch.ps1 -Run @('D:', 'SETUP')
```

Then use the arrow keys, Return and Esc. Under "Setup hardware", pick Sound
Blaster as the sound card and turn CD music on. Save on exit. The values used
here were Sound Blaster at 220 / IRQ 5 / DMA 1, matching `lem3d.conf`, but the
exact choices in setup were not recorded. Re-run setup if sound is wrong.
Setup also has a key-configuration page that lists the game's default key
bindings.

The CD mounts through `IMGMOUNT D "<cue>" -t iso`. DOSBox-X reports one data
track and 23 audio tracks. **[unverified]**: CD music actually playing (no
audio check was possible).

## 2. The harness: `tools/original/`

All scripts are PowerShell 5.1 (`powershell -File …` or `& path\script.ps1`).
They share `Lem3dCommon.ps1` (defaults and helpers) and `Lem3dWin32.cs`
(Win32 input and capture, compiled at run time).

| Script | Purpose |
|---|---|
| `launch.ps1 [-SkipIntro] [-Run cmds] [-WaitSeconds n] [-Force]` | Starts DOSBox-X with `lem3d.conf` plus a generated config (mounts, capture folder, autoexec) and waits for the window. Default `-Run` is `D:` then `L3D`. `-SkipIntro` skips the intro videos and ends on the main menu (about 20 s). |
| `send.ps1 <actions…>` | Sends keys and mouse actions to the window (see below). |
| `shot.ps1 [-Name n] [-Width w] [-Method auto\|print\|screen]` | Saves a PNG of the emulated screen to `LEM3D_SHOTS` and prints the path. `-Width 640` gives one pixel per game pixel. The default `print` method works even when the window is covered. |
| `start-level.ps1 [-Rating Fun\|Tricky\|Taxing\|Mayhem] [-CurrentRating …] [-Code X] [-RowY y] [-Paused]` | From the main menu, opens a level list, clicks a row, continues through the briefing and waits until the level runs. |
| `quit-level.ps1 [-WaitSeconds 15]` | Nukes the level, waits for the results screen and returns to the main menu. |
| `burst.ps1 -Name n [-Count c] [-IntervalMs ms] [-Width w]` | Grabs a timed series of screenshots (down to about 17 ms apart) as `n-0000.png`… plus `n.csv` with each grab time. Use it for timing measurements. |
| `quit.ps1` | Closes DOSBox-X (WM_CLOSE, then kill after 5 s). |
| `calibrate-mouse.ps1` | Re-measures the mouse scale factors (see §5). Run it on a static screen. |
| `lem3d.conf` | DOSBox-X base config: SVGA S3, 16 MB, dynamic core, `cycles=max`, SB16, 1280x960 window at (40,40), no menu bar, no quit prompt. |

### `send.ps1` actions

Each argument is one action, executed in order:

```
ENTER  ESC  F1  P  KP8  UP  SPACE …   tap a key (set-1 scan codes via SendInput)
DOWN*3                                tap three times
LALT+Q                                chord (hold LALT, tap Q)
hold:KP4:800                          hold a key for 800 ms
down:KEY / up:KEY                     press or release only
type:NASTALK                          type text (US layout)
wait:1500                             sleep
move:X,Y   click:X,Y   rclick:X,Y     pointer to game pixel X,Y (640x480 space) [+ click]
lhold:X,Y,MS                          press and hold the left button there
ldown / lup / rdown / rup             press or release a button where the pointer is
home                                  pointer to (0,0)
mrel:DX,DY                            move by DX,DY game pixels (no homing)
nudge:DX,DY                           raw host-pixel motion
```

Use `-TapMs` and `-GapMs` to tune key-hold and inter-action delays (defaults
80 and 120 ms).

## 3. Quick recipes

Boot to the main menu, start the first Fun level paused, and take a
screenshot:

```powershell
$o = 'C:\Users\danle\source\openlem3d\tools\original'
& $o\launch.ps1 -SkipIntro
& $o\start-level.ps1 -Rating Fun -Paused        # menu card starts on Practice after boot
& $o\shot.ps1 -Name fun01 -Width 640
```

Move the camera, take another screenshot, leave the level, quit:

```powershell
& $o\send.ps1 'hold:A:400' 'hold:KP8:600'
& $o\shot.ps1 -Name fun01-moved -Width 640
& $o\send.ps1 P                  # unpause first, so the nuke can run
& $o\quit-level.ps1              # back on the main menu
& $o\quit.ps1
```

The first Tricky level (overall level 21, "Jelly Climber") starts with
`start-level.ps1 -Rating Tricky`. After returning to the menu, the rating card
keeps its last setting, so pass for example `-CurrentRating Tricky` next time.
**[verified]**

### Reaching a given level

Only unlocked levels appear in each rating's list. On a fresh `C:` drive, that
is just level 1 of each rating. **[verified]** for Fun and Tricky. The four
ratings are Fun, Tricky, Taxing and Mayhem. The briefing for Tricky 1 shows
"Level 21", so the levels are numbered overall in blocks of 20 per rating.
**[verified]** for that one data point.

Reaching any other level needs a valid level code: `start-level.ps1 -Code
XXXXXXXX -Rating …`, then click the right row with `-RowY` (row spacing not yet
measured). **No level code has been verified yet.** Two codes from third-party
lists were tried and had no visible effect:

| Code | Claimed level | Source | Result in game |
|---|---|---|---|
| `NASTALK` | 2 | cheatbook.de, "lemm3d.htm" | Typed and accepted with Return, back to the menu, Fun list unchanged. **Not verified.** |
| `STARTING` | 2 | megagames.com, "Lemmings 3D – Level Passwords" | Same. **Not verified.** |

These lists may belong to another edition, or a code may take effect somewhere
other than the level list. The most promising next step is to complete a level
and note the code the game itself shows.

## 4. Navigation reference

### Boot sequence

1. DOSBox-X starts the game. It loads for about 3 s with a black screen.
2. A run of intro videos follows (gears, the Clockwork Games logo, a
   "presents in association with" card, then more). Watched in full, it runs
   for well over a minute.
3. **Esc skips straight to the main menu** once a video is playing.
   **[verified]** Esc pressed during the initial load or between clips is
   ignored. Space seemed to skip only to the next clip **[unverified]**.
   `launch.ps1 -SkipIntro` handles this. It waits 8 s, then sends `Esc, N`
   four times and a final `N`. On the menu, Esc opens a quit confirmation and
   N dismisses it, so the sequence is safe whatever the timing.

### Main menu **[verified]**

Five cards along the bottom of the 640x480 screen:

| Card | Key | Click (x,y), unverified | Effect |
|---|---|---|---|
| Play | F1 | ~(45,400) | Opens the level list for the current rating. |
| Code | F2 | ~(190,400) | "Enter Password" screen: type letters, Return. |
| Options | F12 | ~(285,400) | Configuration screen (see below). |
| Rating | Up / Down | – | Cycles Practice → Fun → Tricky → Taxing → Mayhem (Up goes forward, Down back; it wraps). |
| Exit | Esc | ~(570,400) | "Confirm Y/N" box. Y quits to DOS, N cancels. |

- **Level list** ("Select Fun Level To Play"): one row per unlocked level,
  showing number, name, best lemmings saved and best time. Row 1 is at y≈101.
  Clicking a row opens the briefing. Esc returns to the menu.
- **Briefing** (title card with level number and name): left click = Continue
  (start) and right click = Menu, both [verified]. Return = Preview, as labelled on screen [unverified].
- **Options** (F12, also available inside a level): textures for land, sea and
  sky; replays; "3D axis"; left-handed; video mode; and sliders for CD music,
  music, effects, window, mouse and camera. Click "Exit and Save" at about
  (550,450) to leave. "Default config" is at the bottom left.
- **Practice** (rating card on Practice, then F1): a different screen with an
  EXIT button at about (562,385). **[verified]** It is "Select Item to
  Practice", a grid of 20 icons for the Practice levels (`LEVEL.080`–`099`),
  all available from the start. Row 1 at y≈110: blocker, turner, bomber,
  builder, basher, miner and digger, at x≈75, 160, 225, 320, 405, 470 and
  555. Row 2 at y≈240 starts with climber (75) and floater (160). Row 3 is
  at y≈380; (160,380) is "Slippery Block". Hovering shows an item's name at
  the bottom. Clicking an item opens its briefing (left click = Continue).
  `quit-level.ps1` returns from a Practice level to this screen.

### In a level

Screen layout (640x480 game pixels, approximate centres):

- top left: overhead mini-map;
- top right: counters ("IN", "OUT", time remaining). The exact meaning of IN
  and OUT is still unconfirmed;
- right column, top to bottom: nuke bomb (540,143), fast-forward arrow
  (590,143), camera icon with the current camera number (538,205), two hand
  icons (unknown), two rotate arrows (543,262) and (592,262), release rate −
  (545,318) and + (592,318) with values below them, and the floater/umbrella
  icon (580,370);
- far right: a vertical bar with a yellow marker that tracks camera height;
- bottom row: an arrow at (24,440) (red normally, green in the virtual-lemming
  view), the virtual-lemming face at (68,440), then the skill icons at y≈440
  starting at x≈122, about 52 px apart. Each skill shows its remaining count.

| Action | Input | Status |
|---|---|---|
| Pause / unpause | P | **[verified]** The timer stops. Camera keys, skill selection and the options screen still work while paused. |
| Move camera forward / back | KP8 / KP2 (hold) | **[verified]** |
| Turn camera left / right | KP4 / KP6 (hold) | **[verified]** The view turns. |
| Rotate | A / S (hold) | **[verified]** The view rotates and the sky pans sideways. |
| Raise / lower camera | KP+ / KP− (hold) | **[verified]** The height marker on the right bar moves. |
| Diagonal moves | KP7 / KP9 / KP1 / KP3 | **[unverified]** (public sources) |
| Cursor keys | arrows | Move the camera too. **[verified]** that Left changes the view. Exact mapping not pinned down. |
| Switch camera | 1 / 2 / 3 / 4 | **[verified]** for 1 and 2. The camera icon number changes and the view jumps. |
| Cycle cameras | C | **[unverified]** (public sources) |
| Select a skill | click its icon in the bottom row | **[verified]** A crosshair marks the selected icon. |
| Select a skill by key | F1–F9 | **[unverified]** In a test, F1/F2/F5/F9/F10 showed no selection mark. |
| Assign a skill | click a lemming with a skill selected (game running) | **[verified]** The skill's count dropped from 2 to 1 and the lemming took the skill. |
| Inspect a lemming | hover over it | **[verified]** A bracket cursor appears and its state shows at the bottom left (e.g. "Walker"). |
| Virtual lemming ("lemming cam") | click the face icon (68,440), then click a lemming while the game runs | **[verified]** The view switches to just behind that lemming, and the bottom-left arrow turns green. |
| Leave the virtual-lemming view | I | **[verified]** Back to the normal camera. Public sources say I also toggles it on. **[unverified]** |
| Release rate | hold the + / − icons | **[verified]** (80 → 99 by holding + for 2.5 s) |
| Fast-forward | click the arrow at (590,143) | **[verified]** The icon turns into a green double arrow and the game runs much faster. Click again to turn it off. Keyboard key **[unverified]**. |
| Nuke | Alt+Q | **[verified]** All lemmings get countdowns and explode, then the results screen appears. |
| Esc | Esc | **[verified]** Restarts the level as a **replay** of your actions ("Replaying"). A click takes over control ("Click to Play"). It does **not** leave the level. |
| Options | F12 | **[verified]** |
| Leave the level | nuke, then right click "Menu" on the results screen | **[verified]** (`quit-level.ps1`). A left click there is "Retry", which goes back to the level's briefing. |
| Window size | `{` / `}` | **[unverified]** (public sources) |

**Cheat** (source: cheatbook.de, "lemm3d.htm"): typing `RASPUTIN` while a
level runs turns the pointer into a target. Clicking lemmings then kills them
("Assassinated"), and the results screen says "You Slaughtered N". **[verified]**
That this lets you pass a level (the source's claim) is **[unverified]**: our
best try got 61 of the 79 needed on Fun 1.

### Camera constraint **[verified]**

No key tilts the camera up or down. Rotating, moving and raising or lowering
the camera all keep the horizon on the same screen row (about y≈143 of 480 in
the normal view). The sky above it is a flat band. Rotation slides it sideways
like a panorama, and raising the camera does not change it. Only the 3D world
below the horizon changes with height.

## 5. Quirks and gotchas

- **Mouse input is relative only.** The game ignores DOSBox-X's
  absolute-position integration, so its pointer only moves while DOSBox-X has
  the mouse **locked**. `send.ps1` locks it with DOSBox-X's own hotkey,
  **Ctrl+F10** (a toggle), when needed. Lock status: while the window has
  focus, the host cursor is clipped to the client area. The title bar also
  shows "[Ctrl+F10 releases mouse]", but it can lag the toggle by seconds, so
  it is only a fallback signal. **Alt chords (such as Alt+Q) drop the lock.**
  The next mouse action re-locks it. A warning "Could not confirm that DOSBox-X
  locked the mouse" sometimes appears even though the action worked. Check the
  screenshot.
- **How absolute positions work.** `move`/`click` push the pointer into the
  top-left corner, then move it a calibrated distance, in steps of at most 40
  host pixels. Each step waits for SDL to warp the cursor back to the centre.
  A click takes about 1.5 s. Calibration with the game's default mouse speed
  setting: `PxPerHostX=1.59`, `PxPerHostY=1.905`, offset about (−3,−4).
  Accuracy is a few pixels. If the in-game Mouse speed slider or DOSBox-X's
  `sensitivity` changes, run `calibrate-mouse.ps1` on a static screen (for
  example the Options screen) and pass the new values to `send.ps1`.
- **Stuck keys.** A press whose release is lost stays held system-wide, and
  Windows then auto-repeats it into whatever window has focus (this once
  flooded the user's terminal). `send.ps1` always sends releases and releases
  everything it pressed when it exits, but a killed script can still leave a
  key held. **Run `check-keys.ps1` after every input sequence**: it reports
  and releases any held key or mouse button.
- **Focus.** Synthetic keys and clicks go to whichever window is in the
  foreground. `send.ps1` brings DOSBox-X to the front first, and the helper
  refuses to send any event while another window has focus: the script fails
  with "DOSBox-X is not the foreground window" rather than typing into
  something else. Avoid typing into other windows while a sequence runs.
  Screenshots (`print` method) don't need focus.
- **DPI.** The host runs at 150 % scaling. The helpers make themselves
  per-monitor DPI-aware, so all coordinates are physical pixels. Don't drop
  that call.
- **Timing.**
  - About 3 s from launch until the game shows anything.
  - Esc on the intro works only once a clip is playing.
  - Screen changes (menu → level list → briefing → level) take 2–3 s each.
    Level loading takes about 3–4 s.
  - DOS games poll the keyboard, so taps shorter than about 50 ms can be
    missed. Hold camera keys (`hold:KP8:600`) rather than tapping them.
  - The results screen after a nuke takes 10–15 s on small levels. Use
    fast-forward to speed it up.
- **Esc in a level is not "quit".** It restarts the level as a replay. Use
  the nuke route.
- **The rating card is sticky** while the game runs. It resets to Practice
  only when the game restarts.
- **Restarting cleanly.** `quit.ps1` then `launch.ps1`. If a script dies
  midway, `quit.ps1` still finds the process through the state file.
- **DOSBox-X's own screenshots.** `captures` points at `LEM3D_SHOTS`, but no
  capture hotkey is wired into the scripts **[unverified]**. Use `shot.ps1`.
