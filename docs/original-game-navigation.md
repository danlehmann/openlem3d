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
$o = '.\tools\original'   # from the repository root
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

Other levels are reached by completing the previous one (the next level then
appears in the list; row 2 is at y≈135, so rows are about 34 px apart
**[verified]**) or with a level code. A valid code goes straight to that
level's briefing **[verified]** with `BLIMBING` (level 2) and `FANAGALO`
(level 3), typed through DOSBox-X `AUTOTYPE` (see §6).
`start-level.ps1 -Code XXXXXXXX` types the code and continues from the
briefing; that script path is **[unverified]** (not run since the change). The results screen of a completed level
shows the code of the next one ("Password :-"; buttons "Next Level" on the
left, "Menu" on the right). Verified codes are listed in
[spec/level.md](spec/level.md#level-codes).

Codes from third-party lists don't work in this edition:

| Code | Claimed level | Source | Result in game |
|---|---|---|---|
| `NASTALK` | 2 | cheatbook.de, "lemm3d.htm" | No effect; back to the menu. |
| `STARTING` | 2 | megagames.com, "Lemmings 3D – Level Passwords" | No effect. The game's own code for level 2 is `BLIMBING`. |

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
  at y≈380; (160,380) is "Slippery Block" and (235,380) "Rope Slide"; (555,240) is "One Way". Hovering shows an item's name at
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
  Verified clicks: blocker 122, turner 174, builder 278, basher 330, miner
  382, digger 434 and floater 585 (the umbrella lemming at the far right;
  538 is not the floater).

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

## 6. When the desktop is locked: input without focus

`send.ps1` needs DOSBox-X in the foreground. While the Windows session is
locked, the foreground window is the lock screen and every send fails with
"Could not bring the DOSBox-X window to the foreground" (no input is sent,
so nothing leaks). Posting window messages (`PostMessage` with key or mouse
messages) has no effect on DOSBox-X either. The following works without
focus and was used for the level-code, music and exit measurements
**[verified]**:

- **Screenshots:** `shot.ps1 -Method print` keeps working, but only with
  `output = surface` in an extra config; with the default `direct3d` output
  the grabs stop updating while the session is locked.
- **Extra config files:** `launch.ps1 -ExtraConf a.conf,b.conf` loads them
  after the generated config.
- **Keyboard:** DOSBox-X's `AUTOTYPE` command, run before the game in
  `launch.ps1 -Run`, types a fixed key script into the game later, for
  example `AUTOTYPE -w 8 -p 0.5 esc , n , ... f2 , b l i m b i n g , enter`
  then `L3D` (`-w` initial delay, `-p` pause per key, `,` an extra pause;
  `AUTOTYPE -list` prints the key names). Timing is blind, so about one run
  in three lost the sequence; check with a screenshot.
- **Mouse:** a serial mouse on a TCP "null modem". Add
  `[serial]` / `serial1 = nullmodem port:5555 transparent:1`, load the
  FreeDOS CuteMouse driver before the game (`CTMOUSE /S1 /M /R11`: COM1,
  Mouse Systems protocol, highest resolution; GPL, downloaded separately
  from the FreeDOS repositories and not kept in the repo), then connect to
  `127.0.0.1:5555` and write 5-byte Mouse Systems packets
  (`0x80 | inverted buttons`, dx, −dy, 0, 0; buttons L = 4, R = 1). With
  `/R11`, one count moves the game pointer about 2.1 px in x and 2.5 px in
  y. Moving far up-left first homes the pointer at the top-left corner, so
  absolute positions are reliable to a few pixels. Clicks register when
  packets are spaced about 50 ms apart.
- **CD audio log:** a `[log]` section with `logfile = …` and `misc = debug`
  records each CD-audio play request as `CDROM: Playing track # N`.

Positions found with the serial mouse (640×480 game pixels): skill icons
blocker 122, turner 174, digger about 460 (y 440); camera icon (538,205);
fast-forward (590,143); results screen "Next Level"/"Retry" (60,450) and
"Menu" (600,450); main-menu cards Play (65,400), Code (190,400),
Options (320,400), rating card (455,400, a click steps the rating forward).
Clicking the nuke icon (540,143) worked only sometimes.

Further findings from the code and camera runs **[verified]**:

- **Mouse clicks skip the intro.** Left clicks every 2 s at a harmless spot
  such as (366,232) reach the main menu in about 22 s, with no keys needed.
  A click on the Code card (190,400) then opens the code screen.
- **AUTOTYPE pacing.** With `-p 0.5`, each key or `,` took about 0.8–0.9 s
  of real time, and the start of the script drifted by up to about 10 s
  between runs. Key scripts that must hit a particular screen therefore need
  slack, and the mouse side should synchronise on what is on screen: for
  example, poll screenshots until the briefing appears (bright "Continue"
  text around (90–170, 430–445)), then shift all later timings by the
  measured offset. The briefing waits for its click, so a late click is safe.
- **Camera views.** P (pause) and the camera keys 1–4, typed by AUTOTYPE,
  work while the level runs or is paused. The camera icon shows the current
  camera number in white near (513,200); grouping polled frames by that
  digit separates the four views.
- **Camera moves by AUTOTYPE.** Taps of `kp_8` (AUTOTYPE key name) move
  the camera forward even while paused: 8 taps took Mayhem 6's camera 1
  more than 4 units forward. Taps of `a` turn the view in large steps.
  Taps of `kp_4` and `kp_6` had no visible effect. `AUTOTYPE -list` prints
  the key names. A `-w` of 50 typed nothing; 30 works, so pad longer
  delays with `,`.
- **CD check.** A host folder mounted as the CD (`MOUNT D dir -t cdrom`) is
  rejected: the game prints "Please insert 3D Lemmings CD in drive and close
  door" and exits. A CUE sheet whose data track is a 2048-byte-sector copy of
  the original data track (`MODE1/2048`) and whose audio tracks point into
  the original `.bin` is accepted (DOSBox-X reports its 23 audio tracks).

Findings from the screen-layout runs (`spec/ui-graphics.md`, "Screens")
**[verified]**:

- **Game resolution.** Menus and levels run at 320×200. `shot.ps1` and
  `burst.ps1` (print method, `output = surface`) capture a 720×540 client
  area with the 640×480 picture at (40,30); `l3d-tool ui-find --screen
  40,30,640,480 --game 320,200` maps a grab back to game pixels.
- **Serial-mouse mapping at 320×200.** After homing, (X + 5.5)/2.1 counts
  right and (Y + 6)/2.5125 counts down (X, Y in the 640×480 units used
  above) put the pointer cell's top-left at about (0.4935·X − 4.7,
  0.4135·Y − 7.5) in game pixels.
- **Clicks need a hold.** Clicks held 120 ms were often missed on panel
  icons; holding the button about 300 ms worked every time.
- **No focus needed.** `launch.ps1` brings DOSBox-X to the front only with
  `-SkipIntro`, which types keys. Without it the window is never focused,
  so this setup runs next to someone else using the desktop.
- **Assigning skills to lemmings failed with the serial mouse** (Practice
  "Slippery", turner). Panel clicks work. Over a lemming the pointer turns
  into the bracket and the state name shows ("Walker", "Sliding"). But
  clicking never put a turner's first-click arrow over the lemming, and the
  skill count did not drop. Tried: 120 ms and 300 ms holds when the bracket
  shows, and frame-exact presses using Pause emulation. **[verified failure]**
  Twenty blind 300 ms clicks with a blocker selected on Practice "Blocker"
  failed too. Likely cause: a click in the view that misses a lemming moves
  the camera (away from the middle the pointer is a white arrow pointing
  outwards; a crosshair in the middle; the bracket over a lemming), so
  repeated clicks shift the scene and the lemming leaves the pointer.
- **A single click timed on the bracket assigns** **[verified]** (Practice
  "Blocker", serial mouse, no focus). With the blocker selected and the
  pointer parked on the path, poll grabs until the white arrow under the
  pointer gives way to the bracket, then press once for 100 ms: the caption
  read "Blocker", the count dropped from 5 to 4 and the lemming stood with
  its arms out. Note that `shot.ps1` grabs and the raw 720×540 client grabs
  differ in scale (640 vs 720 wide), which matters when choosing the polled
  spot.
- **Skills can be given through Hi-Light** **[verified]** (Practice "Blocker",
  serial mouse, no focus). Click the panel's red down arrow at about
  (23,440): it turns green and the lemming nearest the middle of the view is
  highlighted. Clicking a skill icon (blocker at (122,440)) then gives that
  skill to the highlighted lemming at once: the count dropped from 5 to 4.
  The arrow again switches highlighting off. Which lemming gets it depends on
  the view (`spec/ui-graphics.md`, "Highlight arrow").
- **A turner by mouse** **[verified]** (Practice "Turner", serial mouse, no
  focus). Select the turner (174,440), click the lemming once on the bracket
  (100 ms), then move the pointer about 60 px to one side (`rel:-29,0`) and
  press 100 ms again: the count dropped and the caption read "Turn right".
  Presses of 300 ms were ignored. Through Hi-Light, clicking the turner icon
  alone gave no turner. The lemming view then rides with it after the face
  (68,440) and a 100 ms press on the standing turner.
- **Screenshot vs mouse coordinates.** In 640-wide `shot.ps1` grabs of a
  level, panel items sit about 32 px right of and 15 px above the
  serial-mouse positions above (the arrow shows at (55,425), the face at
  (100,425)).
- **Level demos without focus.** Each Practice briefing offers "Enter =
  Demo", which replays a recorded solution, including its camera moves
  ("Demo" shows at the bottom left of the panel). The demo ends with a nuke
  and returns to the briefing. Recipe: launch with `-State <main-menu
  state>` and put `AUTOTYPE -w 30 -p 0.5 enter` before `L3D` in `-Run`.
  The queued key survives the state load. Then use the serial mouse to
  reach the briefing (Play, then the item; it is up about 17 s after
  launch), and the Enter fires at about 33 s. **[verified]** for five
  Practice levels.
- **Attract mode.** About 125 s after (re)entering the main menu a demo
  level starts, whatever the mouse does; a click returns to the menu.

## 7. Save states: skip the boot, intro and navigation

DOSBox-X's own save states make a run start where it is needed (the main
menu, a paused level, a code screen) instead of booting, skipping the intro
and navigating there every time. No keys are typed: the scripts drive
DOSBox-X's native menu bar with window messages, which works without focus
and with the desktop locked.

| Script | Purpose |
|---|---|
| `menu.ps1 -List` / `-Item <text>` | Lists DOSBox-X's menu commands, or invokes one by name (e.g. `Save state`). |
| `state.ps1 -Save <name>` | Snapshots the running game to `temp/states/<name>.sav`. |
| `state.ps1 -Load <name>` | Restores a snapshot into the running DOSBox-X. |
| `state.ps1 -List` | Lists the saved states. |
| `launch.ps1 -State <name>` | Launches DOSBox-X and restores the state once it is up. |

`launch.ps1` always enables the menu bar (`showmenu = true`, outside the
game's client area, so capture coordinates don't change) and points
DOSBox-X at `temp/states/current.sav`; `state.ps1` copies named states in and
out of that file.

**Verified:** saving at the main menu and restoring it into a fresh DOSBox-X
brings the menu back 9 s after launch (about 20 s of boot plus a scripted
intro skip before). The restored screen matches the saved one (the button
row at zero offset; the remaining difference is the animated backdrop and
blinking faces).

**Gotchas:**

- A load sent in the first second after launch is ignored; `launch.ps1
  -State` waits 6 s first.
- States belong to this DOSBox-X build and configuration. After changing
  either, make them again.
- A state includes the game's state on disk only as far as DOSBox-X keeps it
  in memory; the save file `LM3D.SAV` in `temp/dosbox_c` is not rolled back
  by loading a state.
- DOSBox-X's log (`[log] logfile = …` in an extra config) records
  "Loading state from slot …" and "Loaded." for each load, which is the
  quickest way to confirm a load happened.

- **Only main-menu states reload reliably.** With the serial-mouse setup
  (§6: `surface`, null-modem serial, CTMOUSE), states saved on the Practice
  grid, on a briefing or inside a level (about 2 MB each) were written but
  silently not restored: the log shows "Loading state from slot 1" with no
  "Loaded.", and the screen does not change. This happened both in the
  running DOSBox-X and in a fresh one. A main-menu state (about 1.2 MB)
  restores every time. Cause unknown. **[verified]** Workaround: restore the
  main-menu state, then click through to the level (about 8 s).
- **Pause emulation.** `menu.ps1 -Item 'Pause emulation'` toggles DOSBox-X's
  pause without focus. `shot.ps1 -Method print` still grabs the frozen
  frame. Serial-mouse packets sent during the pause are queued and applied
  on resume. **[verified]**

**Suggested library** (create on first use, name by content): `main-menu`,
`code-screen`, and `<level-file>-paused` for levels used in comparisons, for
example `level000-paused`.
