# User-interface graphics (`GFX/`, `BMPS/`)

Sources: [L3DEdit] `title mhc format.txt` (namida, also posted as reply #59
in [LF1590]), [L3DEdit] `Graphics.txt`, Pooty's file notes in [LF1590]
(`BombNumb.gfx`, `Cogs.gfx`, `Lemmings.fnt`, `Minilemm.gfx`). Everything
else here was worked out from the data.

Unless stated otherwise, files are headerless 8-bit images: one byte per
pixel, a palette index, rows top to bottom. `.RNC` files are RNC-packed
([rnc.md](rnc.md)); sizes below are after unpacking. The palette is
`GFX/LM3D.PAL` ([graphics.md](graphics.md)), and index 0 is the transparent
background of sprites.

Parsers live in `l3d-formats` (`title`, `font`, `icons`, `sheets`, `screen`,
`image`). `l3d-tool ui --out DIR [PART…]` renders everything below as PNG
contact sheets. `l3d-tool ui-find` finds them in screenshots (see
[Screens](#screens-layout-and-behaviour)).

**How we verified it**, unless a section says otherwise: the sizes add up to
the file length exactly, and the rendered sheets show clean, complete
pictures with no shear or wrap-around. What a picture *is used for* in the
game is **unverified** unless stated; it is inferred from the picture and
from a screenshot of the original's game screen.

## `GFX/TITLE.MHC`: spinning logo (verified)

Not related to `LEMM.MHC`. The file is a sequence of cells with no header
or index ([L3DEdit]):

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | Cell size in bytes, including this field |
| 2 | size − 2 | Run-length-encoded pixels |

Pixel encoding: a non-zero byte is one pixel. A zero byte is followed by a
count `n`: `n` transparent pixels (index 0), or, if `n` is 0, transparent
pixels up to the end of the current row.

The sources don't give the dimensions. **Verified:** with a row width of 256,
all **49 cells** in the file decode to exactly 256 × 64 pixels, no zero run
crosses a row end, and the cell sizes add up to the file length (272,600
bytes). The frames show the red "3D" and green-and-yellow "Lemmings" logo
turning once around its vertical axis (frame 0 face-on, frame 12 edge-on,
frame 24 from behind, frame 48 face-on again). Palette `LM3D.PAL`; colours
look right.

API: `title::decode_rle_cells(data, title::LOGO_WIDTH) -> Vec<IndexedImage>`.

## `GFX/TITLE.RNC`: main-menu buttons (verified)

29,440 bytes in three parts, each a run of equal cells stored one after
another:

| Offset | Cells | Content |
|---|---|---|
| 0 | 5 × 64×64 | A lemming holding up a sign, one per menu entry: **Play** (mouse, F1), **Code** (safe, F2), **Options** (question mark, F12), difficulty rating (graph, up/down arrows), **Exit** (computer, Esc) |
| 20,480 | 5 × 32×32 | Rating signs: **Practice** (L-plate), **Fun**, **Tricky**, **Taxing**, **Mayhem** (graphs of rising steepness) |
| 25,600 | 15 × 16×16 | Lemming faces: 5 heads × 3 frames (eyes open, half closed, closed) |

Each button has a hole of index 0 where the face goes. Its opaque edge
spans columns 32–51 and rows 5–20, larger than a 16×16 face, and the faces
are drawn behind the buttons: head *k* sits in button *k* at the offsets
in [Title screen](#title-screen-main-menu) (verified in the running
game). The rating signs fit the board on the fourth button.

API: `title::MenuArt::parse(data)` with `buttons`, `ratings`, `faces`.

## `GFX/TITLE.FNT`: menu font (verified)

23,240 bytes = **166 glyphs of 10×14**, stored one after another, for
character codes **33 (`!`) to 198**: glyph `i` is code `33 + i`.

- Codes 33–122 are an italic ASCII font, white and grey on index 0. Glyphs
  `0`–`9`, `A`–`Z`, `a`–`z` and punctuation sit at their ASCII codes
  (checked: `0` is glyph 15, `A` glyph 32, `?` glyph 30, `a` glyph 64).
  Codes 91–96 (`[\]^_` and backtick) are empty.
- Codes 123–198 are not letters but tiles of small pictures, laid out to be
  printed as strings: 123–135 and 136–148 are the two rows of a
  "Psygnosis" wordmark, about 149–176 a small red-and-green "3D Lemmings"
  logo of two rows, and the rest small pictograms (a blue globe and others).
  Exact tile boundaries are **unverified**.

API: `font::title_font(data) -> Font`; `Font::glyph(code)`.

## `GFX/LEMMINGS.FNT`: lemming lettering (verified)

[LF1590] says this file is 32 pixels wide and holds the animated lemmings
of the code screen. **Verified:** 539,648 bytes = **527 frames of 32×32**:

| Frames | Content |
|---|---|
| 0–5 | Idle: a row of lemmings standing and shuffling |
| 6 + 20·k … 25 + 20·k | Letter `A + k` (k = 0–25): starting from the row, the lemmings climb on each other to form the letter, hold it, then drop back into the row |
| 526 | The row again |

Every letter's first frame is the row of standing lemmings (all frames
`6 + 20k` have no opaque pixel above row 22), which is how the 20-frame
period was found. Only letters A–Z exist, no digits. The code screen
plays a letter's frames 0–15 and then loops 12–15; 16–19 play on
Backspace ([Code screen](#code-screen-f2)).

API: `title::LemmingLetters::parse(data)`; `.idle()`, `.letter(b'A')`.

## `GFX/ICONS.RNC`: panel icons and small fonts (mostly verified)

63,961 bytes. The file is neither planar nor a list of sized sprites: it
is several parts, each of fixed-size cells stored one after another, placed
back to back with no header. The cell size changes between parts. Only the
part boundaries had to be found; rendering each part at its own width shows
clean pictures.

| Offset | Size | Cells | Content |
|---|---|---|---|
| 0 | 14,592 | 57 × 16×16 | Small icons: 12 block shapes (cube, slopes, wedges); level-editor buttons (blank, A–I, outline, ADD/DEL/MOV, magic wand, grid, colour up/down); action pictures (storm cloud, eye, lemmings in skill poses, explosion, floater); "CLOCKWORK" (two cells); four white arrows; left and right mouse buttons |
| 14,592 | 26,496 | 46 × 24×24 | In-game panel icons, see below |
| 41,088 | 2,560 | 32×32 + 3 × 32×16 | Umbrella; "IN" and "OUT" spelt in lemming lettering; an alarm clock |
| 43,648 | 4,473 | — | Wooden panel pieces, **not decoded** (see below) |
| 48,121 | 5,040 | 90 × 7×8 | Small font, codes 33–122 |
| 53,161 | 10,800 | 90 × 10×12 | Large font, codes 33–122 |

**Panel icons (24×24).** Matched against a screenshot of the original's
game screen, which shows the bomb, camera, paws, turn arrows and −/+ in the
right-hand column, the "IN"/"OUT" labels with the clock above them, and a
red down arrow and the umbrella among the skill buttons:

| Cells | Picture |
|---|---|
| 0–13 | Brown paw prints walking: an animation, 14 frames (pause) |
| 14, 15 | `+`: green, red |
| 16 | `>>` (green) |
| 17 | `>` (small, red) |
| 18, 19 | Clockwise turn arrow: green, red |
| 20, 21 | `−`: green, red |
| 22, 23 | A lemming's face: eyes open, squinting |
| 24, 25 | Anticlockwise turn arrow: green, red |
| 26 | Black bomb (nuke) |
| 27–37 | A mushroom-cloud explosion growing and fading, 11 frames |
| 38 | Red down arrow |
| 39–44 | Green down arrow, animated, 6 frames |
| 45 | Cine camera |

The red versions are the idle state and the green ones are shown while a
button is held or fast-forward is on ([In-level panel](#in-level-panel)).

**Fonts.** Both are bitmap fonts with glyph `i` = code `33 + i`, ASCII
order, upper and lower case, digits and punctuation. Verified: `0` is glyph
15 and `z` is the last glyph in both. In the 7×8 font, codes 35–37 (`#$%`)
hold three pictures instead: a yellow light, a red light and a grey down
arrow (a slider knob?). The large font matches the counters on the game
screen (lemmings in and out, time); which counters use which font is
**unverified**.

**Not decoded: 43,648–48,120.** All pixels are opaque. Parts render
coherently at 12 pixels wide (vertical wood planks, a yellow-and-red knob,
a strip of water and of sand, matching the 12-pixel slider at the right
edge of the game screen), others don't. 4,473 bytes is not a multiple of 12,
so the part holds more than one piece. The first rows look like a wider
image of horizontal planks.

API: `icons::Icons::parse(data)` with `small`, `panel` (indices in
`icons::panel`), `umbrella`, `labels`, `unknown`, `small_font`, `large_font`;
byte offsets in `icons::offsets`.

## Other sprite sheets (verified)

All are a single strip of equal cells, read with `sheets::Sheet::cut`.
Widths come from the strongest row-to-row correlation and were then checked
by rendering.

| File | Bytes | Cells | Palette | Content |
|---|---|---|---|---|
| `MINILEMM.GFX` | 65,536 | 64 × 32×32 | `LM3D.PAL` | Small lemmings, animated: a fat lemming, a group of lemmings shrinking into the distance, a lemming turning, a climber on a stone wall, a lemming with arms out (blocker), lemmings with a hammer, a brick bag and plank, a pickaxe, and one digging. [LF1590]: the skill icons at the bottom of the game screen. Each skill's cells are listed under [In-level panel](#in-level-panel). |
| `BOMBNUMB.GFX` | 8,192 | 8 × 32×32, two per row (64 wide) | `LM3D.PAL` | Brush-stroke `1`–`5`, `?`, a white down arrow, an empty cell. [LF1590]: bomber countdown, camera number, the `?` of a builder out of bricks, the highlight cursor. |
| `COGS.GFX` | 26,136 | 3 × 88×99 | the palette in `LOADING.RNC` | Three meshing cogs (blue, green, red), 3 animation frames. With `LM3D.PAL` the colours are garbled; with the `LOADING.RNC` palette they are clean shaded blue, green and red. [LF1590] guessed the 88-pixel width. |
| `MOUSE.RNC` | 11,008 | 43 × 16×16 | `LM3D.PAL` | Mouse pointers: turn left, up, turn right, left, down, right, two diagonal turns, a bracket frame, a cross-hair, a target, then a white arrow in 32 directions, turning clockwise from straight up in steps of 11.25°. |
| `DEFLICON.RNC` | 32,768 | 32 × 32×32 | `LM3D.PAL` | A brown wedge-shaped block turning through 32 angles. |
| `PRACICON.RNC` | 57,344 | 56 × 32×32 | `LM3D.PAL` | Object pictures with their animations: a red pad (4), a stone table (2), one-way arrows (4), a lemming on a rope slide (5), a white disc (4), a spring (7), soil (8), a lemming waving (5), a green tick (1), an "EXIT" sign turning round (16). |
| `ENDLEMMS.RNC` | 5,632 | 8 × 22×32 | `LM3D.PAL` | A lemming facing the viewer, raising its arms and turning round. |
| `WINDER.RNC` | 32,768 | 8 × 64×64 | `LM3D.PAL` | A lemming turning a large wheel, side view: the two winders of the title-screen banner (see Screens). |

## Full-screen pictures (verified)

| File | Bytes | Layout |
|---|---|---|
| `LOADING.RNC` | 19,968 | 320×60 pixels, **then** a 768-byte VGA palette |
| `INTROn.RNC` (n = 1–7) | 64,768 | 768-byte VGA palette, **then** 320×200 pixels |
| `INTROn.SVG` | 307,968 | 768-byte VGA palette, then 640×480 pixels |
| `SCENEnnn.RNC` (nnn = 000–010) | 64,000 or 65,024 | 320×200 pixels, palette in `SCENEnnn.PAL`; files 001–010 add 4 × 16×16 prompt cells |
| `SCENEnnn.SVG` | 307,200 or 311,296 | 640×480 pixels, palette in `SCENEnnn.SVP`; files 001–010 add 4 × 32×32 prompt cells |

The `.SVG` files are the high-resolution (SVGA) versions of the same
pictures; they have nothing to do with the vector format of the same
extension. How we verified the palette positions: the 768 bytes at the
named place are all 0–63 and give clean colours, and the other end of the
file is image data.

- **LOADING:** "Now Loading" in green letters held by three lemmings.
- **INTRO 1–7:** the Psygnosis owl and wordmark; "Presents in Association
  With"; Clockwork Games logo with cogs; the "Lemmings 3D" title with a
  sitting lemming; "presented in Dolby Surround"; "Featuring"; the Jelly
  Belly logo.
- **SCENE 000–010:** rendered theme pictures. 000 lemmings in graduation
  gowns, 001 a king and queen at a banquet (castle), 002 a tomb (Egypt), 003
  a spaceship (space), 004 jelly beans (sweets), 005 a golf buggy (golf), 006
  a computer (computer), 007 a drill sergeant (army), 008 a maze exit sign
  (maze), 009 a strongman and clown (circus), 010 a lemming built of toy
  bricks (Lemgo). When the game shows them is **unverified**.
- **Prompt cells** (scenes 001–010): left mouse button, right mouse button,
  and the left and right halves of an "ENTER" key, in the scene's palette.

API: `screen::intro(data, screen::LOW_RES | HIGH_RES)`,
`screen::loading(data)`, `screen::scene(data, size)` (pixels plus
`prompts`; the palette is read separately with `image::vga_palette`).

## `BMPS/800BMPS`, `BMPS/1024BMPS` (verified)

Standard uncompressed 8-bit Windows bitmaps (`BITMAPINFOHEADER`, bottom-up
rows, 256-entry BGRA palette after the header): ten at 800×600 and ten at
1024×768, one per theme (army, castle, circus, computer, Egypt, golf, Lemgo,
maze, space, sweets). They are larger renders of the theme scenes with a
Clockwork Games logo, apparently desktop wallpapers rather than game
graphics. API: `screen::bmp(data)`.

## Screens: layout and behaviour

Observed in the running game (DOSBox-X, `docs/original-game-navigation.md`
§6: AUTOTYPE keys and the CuteMouse serial mouse, no host input). Tags:
**verified** (with the method) or **estimated** (timing read off screenshot
series, or a single observation).

**Method.** Screenshots and bursts (`burst.ps1`, 17–150 ms apart) were
matched against the decoded graphics with `l3d-tool ui-find`, which looks
for every cell of a sprite set (palette index 0 transparent, colours from
`LM3D.PAL`) at 1:1 scale and reports exact positions and how many opaque
pixels differ. "Verified (match)" below means a placement with **0
differing pixels**; a frame sequence marked "verified (burst)" is the
per-frame result of that matching over a burst.

**Resolution.** The title screen, the code screen and the level itself run
at **320×200** (VGA 256-colour) in both video modes the options offer (the
level list and the options screen look the same, checked by eye);
DOSBox-X shows them stretched to 640×480 (columns doubled, rows repeated 2
or 3 times). All coordinates
below are in 320×200 game pixels, origin top-left, and are the top-left
corner of the named cell. `ui-find --screen 40,30,640,480 --game 320,200`
resamples the screenshot back to 320×200 (the capture adds a 40/30-pixel
black border). Verified (match): every sprite listed below matches with 0
differing pixels at 320×200, and none matched at 640×480.

**Palette.** All three screens use `GFX/LM3D.PAL` unchanged. Verified
(match: colours compared within ±12 summed RGB, i.e. exact after 6→8-bit
scaling).

### Title screen (main menu)

Drawing order, back to front (estimated, except that the faces are
verified to lie under the buttons): backdrop, logo, banner and winders,
faces, buttons, rating sign and label, pointer.

| Element | Graphic | Position | Status |
|---|---|---|---|
| Backdrop | `GFX/BGRD.000` (320×48), tiled vertically | x = 0, scrolls (below) | verified (match with `raw:GFX/BGRD.000:320`) |
| Logo | `TITLE.MHC` frame (256×64) | (32, 10), centred | verified (match) |
| Left winder | `WINDER.RNC` cell, as stored | (0, 70) | verified (match) |
| Right winder | `WINDER.RNC` cell, mirrored | (256, 70) | verified (match, `--mirrored`) |
| Banner paper | drawn, not a sprite | x 63–256, y 93–127 | estimated (pixel rows of one shot) |
| Buttons 0–4 | `TITLE.RNC` button *k* (64×64) | (64·*k*, 135) | verified (match) |
| Faces | `TITLE.RNC` face, head *k* in button *k* | see below | verified (match) |
| Rating sign | `TITLE.RNC` rating *r* (32×32) | (207, 153) | verified (match, all five) |
| Pointer | `MOUSE.RNC` cell 9 (cross-hair) | follows the mouse | verified (match) |

**Backdrop.** `BGRD.000` repeats every 48 rows and scrolls **upwards**,
about 7 pixels per 200 ms (≈35 px/s, one pixel every second 70 Hz frame),
wrapping seamlessly. Verified (burst: the tile's vertical phase falls by 7
every ~203 ms over 4 s); rate estimated. There is no `SCENE` picture on
this screen.

**Logo animation.** Frames 0, 1, …, 48 in order (one turn; frame 48
is face-on again), then frame 0 is held for about 1.4 s, then the next turn.
It does not ping-pong. 46 frames took 1.34 s, so about **35 frames/s**
(70 Hz ÷ 2); one cycle lasts about 2.84 s. Verified (burst, 20 ms spacing);
rates estimated. A few grabs show a half-updated frame (tearing).

**Winders.** Each winder lemming turns the roller at its end of the
banner; the rollers are part of the `WINDER` cells (the 64×64 cell covers
x 0–63, the roller is at x 56–62; mirrored at 257–263). The left one plays
cells 7, 6, …, 0 (descending), the right one plays mirrored cells
0, 1, …, 7, so their cell numbers always add up to 8 (mod 8). About 8
cells per 450 ms (≈17.5 frames/s, 70 Hz ÷ 4). Both stop while the banner
holds a page. Verified (burst); rate estimated.

**Banner (credits scroller).** White paper between the rollers. Text is
`TITLE.FNT` codes 33–122 drawn as stored (the glyph colours are the dark
indices 232–255), in up to two lines whose glyph cells start at **y = 97
and y = 111**; each line is centred on x ≈ 160. Verified (match of
individual glyphs). Glyphs are proportionally spaced and overlap (italic),
so the advance per glyph is less than 10 (estimated from glyph positions,
e.g. 5–10 px). Picture pages use `TITLE.FNT` picture tiles (codes 123 and
up): the "Psygnosis" wordmark, a small "3D Lemmings" logo and a
"Clockwork Games" logo (estimated: identified by eye).

Pages enter from the right and move left **2 pixels per step, about 35
steps/s (≈70 px/s)**; when a page is centred the scroll and the winders
stop for about 0.7 s (a second run: about 3.1 s moving, 0.7 s still, about 3.8 s per page), then the page leaves to the left while the next one
follows (verified: burst of a glyph's x position, 226 → 140 in 1.24 s in
steps of 2, then stationary; hold estimated). Page order: wordmark,
"Proudly Presents.", logo, then the game credits as pages of a role line
and a names line, with the Clockwork Games logo between sections. The roll runs to at least 32 pages (design, programming, graphics, level
design, music, production, play testing, special thanks) before attract
mode cut it off; whether it loops was not seen. The
sequence restarts from the beginning whenever the title screen is entered. Ours keeps the three
picture pages and replaces the original team's credits with a few pages of
our own, so the remake does not read as the original publisher's product.

**Faces in the buttons.** Head *k* (`faces` cells 3*k* … 3*k* + 2) sits in
button *k*, drawn *under* the button (the button's opaque border overlaps
it; its index-0 hole shows the face):

| Button | Face cell (open) | Face position | Offset in button |
|---|---|---|---|
| 0 Play | 0 | (26, 140) | (26, 5) |
| 1 Code | 3 | (85, 145) | (21, 10) |
| 2 Options | 6 | (145, 141) | (17, 6) |
| 3 Rating | 9 | (217, 139) | (25, 4) |
| 4 Exit | 12 | (280, 141) | (24, 6) |

Verified (match, all five, in every frame of a 200-frame burst).

**Blinking.** One face at a time, chosen apparently at random, plays open →
half (cell +1) → closed (+2) → half (+1) → open. Observed durations: half
60–80 ms, closed 35–80 ms, half again 35–50 ms (about 2–5 frames at 70 Hz
each). Blinks started 0.6–1.1 s apart (four intervals). Verified (burst:
sequence and which face); timings estimated.

**Rating sign and label.** The sign for the current rating (`ratings` cell
0 Practice, 1 Fun, 2 Tricky, 3 Taxing, 4 Mayhem) is drawn at (207, 153),
i.e. button 3 + (15, 18), covering the "Fun" graph baked into button 3.
Below it the rating name replaces the button's baked "FUN" label: capital
letters 3 px wide and 5 px tall with a 1-px gap (4-px advance), each row
coloured from a red-to-yellow ramp (indices 0x40–0x50), on a black box one
pixel larger than the text on every side, text rows y = 179–183, centred on
the button's centre (x ≈ 224). For "PRACTICE" the text spans x 208–238.
Verified (pixel indices of one shot; sign by match). This lettering is not
in `TITLE.RNC`, `TITLE.FNT` or `ICONS.RNC` (searched for its pixel rows),
so it comes from the program; our implementation needs its own 3×5 font.

A left click on the rating button steps forward Practice → Fun → Tricky →
Taxing → Mayhem → Practice (verified, five clicks, sign matched after
each). A right click does nothing (verified). Up/Down do the same from the
keyboard (see the navigation document).

**Text.** Apart from the banner and the rating label, the title screen has
no text: the button captions ("PLAY", "F1", …) are baked into the button
cells.

**Hover and click.** Hovering over a button changes nothing (verified:
buttons, faces and sign match unchanged with the pointer on Play). A left
click opens Play (level list), Code (code screen), Options (configuration
screen) or steps the rating (verified for Play, Code, Options and the
rating; Exit opens a confirmation, per the navigation document); there is no
pressed-button image. Clicks were only reliable with the button held about
300 ms (estimated; the game seems to poll the mouse once per frame).

**Attract mode.** About 125 s (a second run: about 118 s) after the title screen is entered, a demo
level starts (its briefing shows "Rating Demo", and "Demo" is printed at
the bottom left during play); a click returns to the title. Mouse
movement and clicks on the title did not postpone it (observed three
times). Estimated: in the first stay after boot no demo started within
10 minutes, which is unexplained. Ours plays the Practice demos in turn after
120 s on the title screen without input (any input restarts the wait, so
it never interrupts a choice), and returns to the title on any input or
when the level ends.

### Code screen (F2)

| Element | Graphic | Position | Status |
|---|---|---|---|
| Backdrop | `BGRD.000`, tiled, scrolling up | x = 0 | verified (match) |
| "Enter Password" | `ICONS.RNC` large font | cells at y = 60, x from 117 | verified (match) |
| Letter slots 0–7 | `LEMMINGS.FNT` 32×32 frames | (32 + 34·*k*, 90) | verified (match) |
| "Password Correct" | large font | y = 140, x from 111 | verified (match) |
| "Press Return when finished" | large font | y = 160, x from 85 | verified (match) |

No mouse pointer is drawn (verified: no `MOUSE` cell matched). The
backdrop scrolls up at about 23 px/s here (estimated, burst).

**Large-font spacing.** Each glyph is drawn 1 px left of the previous
glyph's last opaque column, i.e. advance = (last opaque column, counted
from 1) − 1: most letters advance 6, wide ones 7. A space advances 5.
Lines are centred on x = 160. Verified (glyph positions against glyph
widths for "Enter Password").

**Idle animation.** An empty slot loops through 7 images: `LEMMINGS.FNT`
frames 0, 1, 2, 3, 4, 5, then frame 6 (the first frame of "A", the plain
row of lemmings), at about 11.7 images/s (70 Hz ÷ 6). The slots are
staggered: slot *k* is one image ahead of slot *k* − 1, except that slot 7
shows the same image as slot 6. Verified (burst, 100 ms spacing, all eight
slots); rate estimated.

**Typing a letter.** The letter goes into the leftmost empty slot and plays
its frames 0 → 15 (of its 20) once at the same rate, then loops frames
12, 13, 14, 15 for as long as it stays. Verified (burst, letters A, B,
Q–U, B L I M B I N G). Only A–Z exist.

**Backspace** clears the rightmost letter: it plays frames 16 → 19 (the
lemmings dropping back into a row), then the slot returns to the idle loop
in step with the slot to its right. Verified (burst, two backspaces).

**Return.** With a valid code, "Password Correct" appears for about 0.6 s
and the level's briefing follows (verified with `BLIMBING`, level 2;
duration estimated). With an unknown code (`QWERTYUI`) the game returns
straight to the title screen with no message (verified). On the title the
banner then restarts from its first page.

### Screen transitions

Observed (30 ms bursts): from the title to the code screen (F2) the whole
picture fades evenly to black in about 0.6 s, stays black about 0.15 s, and
the code screen fades in in about 0.15 s. Leaving the code screen (Return
with an empty code shows "Password Incorrect" first) fades out in about
0.15 s, stays black about 0.7 s and fades the title in over about 0.42 s;
the banner restarts from its first page. Title to Options (F12) is an
instant cut; leaving Options cuts to black and fades the title in over
about 0.43 s. No wipes or slides. Esc does nothing on the original's code
screen (only Return leaves it); ours also leaves on Esc.

Ours keeps only the fade-ins (the code screen over 0.3 s, standing for the
whole change; the title over 0.42 and 0.43 s), so a screen takes input from
its first frame.

### In-level panel

The panel is not a separate area: every element is a sprite drawn straight
over the 3D view (index 0 transparent; the scene shows between the icons),
except the minimap frame and the slider, which are opaque. Positions are
the same in video modes 1 and 2. Verified (match in levels Fun 1 and 2;
identical placements in both video modes).

| Element | Graphic (`ICONS.RNC` unless noted) | Position | Status |
|---|---|---|---|
| Minimap | frame drawn; map 64×64 | outer (0, 0)–(70, 70); map at (3, 3) | verified (pixel rows). Content (by eye, four captures): a top-down map, two pixels per cell, X to the right and Z downwards; sea in its colour (blue, or red on a lava level), land green, blocks in their colours, the exit red, the hatch orange, lemmings as white dots and a yellow dot for the camera. Measured (Practice levels, exact RGB): sea 0,101,231; grass and grass-topped blocks 48,138,48; grey stone paths topped at 1 unit 77,40,4 (dark brown); other blocks 81,44,4, 89,48,8 and, for a house topped at about 3 units, 93,52,12; exit 247,0,0; hatch 255,162,0; lemmings white; camera 247,231,0. On Fun 1 the tall castle walls show nearly white, so the rule is not a simple height ramp (open). Ours uses the measured exit, hatch and camera colours and colours blocks by their top texture |
| Corner marks | 3×3 red squares | (0, 0), (317, 0), (0, 197) | estimated (pixel rows) |
| "IN" label | labels cell 1 (32×16) | (260, 0) | verified (match) |
| "OUT" label | labels cell 2 | (260, 16) | verified (match) |
| Clock | labels cell 3 | (260, 32) | verified (match) |
| IN / OUT / time | large font, right-aligned | last glyph cell at x = 299; y = 2 / 18 / 34 | verified (match) |
| Nuke | panel 26 | (260, 48) | verified (match) |
| Fast-forward | panel 17 (off), 16 (on) | (284, 48) | verified (match, toggled) |
| Camera | panel 45 | (260, 72) | verified (match) |
| Camera number | small font digit | (269, 80) | verified (match, 1 → 2 on click) |
| Pause paws | panel 0–13 | (284, 72) | verified (match) |
| Turn clockwise | panel 19 (idle), 18 (held) | (260, 96) | verified (match) |
| Turn anticlockwise | panel 25 (idle), 24 (held) | (284, 96) | verified (match, idle) |
| Release rate − | panel 21 (idle), 20 (held) | (260, 120) | verified (match) |
| Release rate + | panel 15 (idle), 14 (held) | (284, 120) | verified (match) |
| Rate numbers | small font, two digits | left at x 267, 272; right at 291, 296; y = 140 | verified (match) |
| Umbrella | labels cell 0 (32×32) | (274, 143) | verified (match) |
| Arrow | panel 38 (red) | (0, 172) | verified (match) |
| Lemming-cam face | panel 23 (idle), 22 (armed) | (24, 172) | verified (match, click) |
| Skill buttons | `MINILEMM.GFX` 32×32 | y = 168, x below | verified (match) |
| Skill counts | small font, right-aligned | last digit cell at (button x + 17, 188) | verified (match) |
| Slider | drawn wood column | x 308–319, full height | verified (pixel rows) |

**Counters.** The large font as on the code screen, digits 7 px apart (8
wide, overlapping 1), right-aligned so the last digit's cell starts at
x = 299. The time is M:SS; the colon is the 4-px-wide `:` glyph, e.g.
"3:53" has digits at 281, 292, 299 (colon at 288, estimated). The left
rate number is the minimum release rate and the right one the current
rate (both 80 at the start of level 2; only the right one changes).
OUT counted up (4 → 59) while the hatch released lemmings; IN shows the
level's requirement before any lemming is saved (79 in Fun 1, 70 in level
2), counts down to 0 as lemmings are saved, then counts those saved beyond
the requirement (observed in the Practice demos).

**Skill buttons.** Order and cells (rest cell = shown when not selected):

| x | Skill | Rest cell | Selected animation |
|---|---|---|---|
| 45 | Blocker | 19 | 19 → 25 → 19, ping-pong (verified) |
| 72 | Turner | 26 | 26 → 32 → 26, ping-pong (verified) |
| 98 | Bomber | 0 | estimated 0–10 |
| 128 | Builder | 46 | estimated 46–51 |
| 158 | Basher | 33 | estimated 33–40 |
| 189 | Miner | 52 | estimated 52–57 |
| 216 | Digger | 58 | 58 → 63, then 58 again (loop, verified) |
| 247 | Climber | 11 | estimated 11–18 |
| 276 | Floater | 41 | estimated 41–45 |

The x positions are not evenly spaced; they are the cells' own positions,
constant through each animation (verified, bursts). Only the selected skill
animates; there is no other selection mark (no frame or highlight;
verified, zoomed shots). A ping-pong cycle took about 200 ms and the
digger loop about 165 ms (≈30–70 cells/s; estimated). At the start of
level 2 the blocker (the first slot, with a non-zero count) was already
selected; the Practice "Bomber", with only bombers, started with nothing
selected (owner's observation). Ours selects the blocker when it has a
count and nothing otherwise (inferred from the two). Clicking a skill whose count is 0 does not select it (verified:
the turner, count 0, left the blocker animating). A count of 0 is shown as
no digits at all (verified); other counts are small-font digits, 5 px
apart, right-aligned at button x + 17 (e.g. "10" with digits at 84 and
89 under the turner).

**Green and red icons.** The red icon is the idle state; the green one is
shown while the button is held (rotate, −, +) or, for fast-forward, while
the mode is on (`>` red off, `>>` green on). Verified (bursts during a
held press; two toggles). Holding + or − changes the current rate by one
per 70 Hz frame (99 → 80 in about 270 ms) with no initial delay, clamped
to 99 and to the minimum (verified, burst; rate estimated). Hovering does
not change any icon (verified for clockwise turn).

**Pause.** While paused (P) the time stops and the paws play cells 0 → 13
in a loop, about 14 cells per 200 ms (one per 70 Hz frame); everything
else is unchanged. When running, the paws show cell 0. Verified (burst
around an AUTOTYPE P). Clicking the paws icon did not pause (two tries).
While paused, skills can be selected but not given to lemmings (owner's
observation); highlighting and the lemming view still work in ours.

**Lemming cam.** Clicking the face changes it from the squinting cell 23
to the open-eyed cell 22 (verified); the next click on a lemming then
enters its view, where the arrow at (0, 172) turns green (cells 39–44,
animated; from the navigation document, not re-measured here).

**Nuke.** Single and double clicks on the bomb had no effect in these runs
(the navigation document also found clicks unreliable; Alt+Q works). Its
animation (explosion cells 27–37) was not observed. The owner recalls that a
single click does nothing and a double click nukes, with the explosion
playing on the icon over and over until the level ends; ours loops it at 14 frames/s.

**Turner arrows.** After the first click with the turner, two white arrows with a thin black outline, about 12 game pixels tall, appear over the lemming, one per direction it could be turned (seen walking across the screen: an up arrow at the head, a down arrow over the feet). They take turns, never shown together, swapping 7–10 times a second, until the second click (observed: Practice "Turner" demo, 50 ms bursts). Ours draws `MOUSE` cells 11–42 picked by each side's screen direction, swapping every 0.12 s; once the pointer is clearly on one side, only that arrow shows, previewing the direction the next click gives (ours; the demo shows no pointer, so the original's behaviour there is unknown).

**Mouse pointer.** Ours is the system's hardware cursor showing these cells, so it moves at the system's rate rather than the game's frame rate. Over the panel icons: cross-hair, `MOUSE` cell 9. Over
the 3D view (and the minimap) the pointer depends on which third of the
view it is in, horizontally and vertically, as a 3×3 grid:

| | left | centre | right |
|---|---|---|---|
| top | 7 | 1 (up) | 6 |
| middle | 0 (turn left) | 9 (cross-hair) | 2 (turn right) |
| bottom | 3 (left) | 4 (down) | 5 (right) |

Verified (match at 15 points, plus boundary probes). The boundaries lie at
about x ≈ 100 and ≈ 200 and y ≈ 55 and ≈ 115 (pointer cell centre),
consistent with thirds of a 308×168 view (estimated). Over a lemming the
pointer becomes a bracket (cell 8, from the navigation document; not
re-measured: no lemming was under the pointer in these runs). What holding a
button there does in the original is unrecorded. Ours: holding the camera
button (right, or left when left-handed) still over the view moves the
camera as the arrow shows (up: forward; down: back; left and right:
sideways; the turn arrows turn; the diagonal turns move forward while
turning); a drag of more than 6 pixels turns the view instead.

**Demos.** Practice briefings offer "Enter = Demo" (verified), which plays a
recorded solution with "Demo" at the bottom left; it ends with a nuke and
returns to the briefing (observed). The original's recordings
(`REPLAYS/REPLAY.080`–`099`) are its own input and can't drive our
simulation, so our demos replay solutions found in it
(`l3d_sim::demos`), end when the level does, and any key or click returns
to the briefing.

**Caption.** Hovering over a lemming names its state in large lettering at
the bottom left ("Walker", "Sliding"; verified, from the navigation
document; the other names and the position, above the arrow and face at
about (3, 158), are ours). Esc restarts the level as a replay of the
player's actions, showing "Replaying"; a click takes over ("Click to Play")
(verified). Whether the two messages alternate or show together is
unrecorded; we alternate them every 1.5 s. Our replay keeps the player's
commands (skills, turner sides, release rate, nuke) with their ticks; any
command of the player's own also ends it. In Enhanced mode ours first asks "Restart level? Y / N" over the paused level (Y or Return restarts, N or Esc carries on); the original asks nothing.

**Video modes.** The options screen offers "Video Mode" 1 and 2 only (a
left click toggles; a right click does nothing). Both run at 320×200 with
the same panel layout (verified: identical placements of all panel sprites
in Fun 1 in mode 1 and level 2 in mode 2). No 640×480 mode was offered by
this setup, although the CD has 640×480 versions of the scenes and intro
slides; how the panel would look there is unknown.

### Configuration screen (F12)

F12 opens it from the title screen and during a level (verified: "Camera
keys, skill selection and the options screen still work while paused");
Ours returns to where it was opened, and the level waits meanwhile.

From one capture (estimated positions, in 320×200 pixels): a blue mottled
backdrop; "Configuration" in a box at the top centre (x 112–207, y 4–20);
two rows of four boxes 80 pixels wide at y 37–51 and 69–83 (Textures,
Land, Sea, Sky; Replays, CD Anims, Left Handed, Video Mode 2), each with a
light at its right end (the small font's yellow code 35 for on, red code
36 for off); three rows of sliders of ten lights at y ≈ 101, 117 and 133
(CD Music, Music, Effects on the left, lights from x 64; Window, Mouse,
Camera on the right, lights from x 224); and "Default Config" and "Exit and
Save" in boxes at the bottom corners (y 180–195). The labels are the small
7×8 font in capitals. Our screen keeps this layout with the settings that
apply to it (Land, Sea, Sky, Left Handed, Fullscreen, CD Music, Effects,
Camera).
### Level list ("Select Fun Level To Play")

From a capture (positions estimated, 320×200): the `BGRD` backdrop; the
title in the white large font at the top centre; the panel's IN label and
clock at the top right (x ≈ 247 and 291, y ≈ 19); and one row per unlocked
level in the large font tinted red: the number right-aligned, the title
from about x = 30, and under the IN and clock icons the best result
("0" and "0:00" for a level not yet played). Rows are about 14 pixels
apart from y ≈ 42. Only unlocked levels are listed in the original; ours
lists all twenty and scrolls.
### Practice menu ("Select Item to Practice")

From a capture (positions estimated, 320×200): the `BGRD` backdrop, the
title in the large font at the top, and three rows of icons centred at
y ≈ 53, 102 and 152. Row 1: the `MINILEMM` rest cells of blocker, turner,
bomber, builder, basher, miner and digger. Row 2: climber and floater
(`MINILEMM`), Hi-Light (the panel's lemming face), Claustrophobic (the
panel's red down arrow), Deflector (`DEFLICON`), Mud (`PRACICON` soil) and
One Way (`PRACICON` arrows). Row 3: Splitter (`PRACICON` stone table),
Slippery (`PRACICON` lemming), Rope Slide, Catapults (the spring),
Trampoline (the white disc), Teleporter (the red pad) and an `EXIT` sign.
Completed items carry a green tick (`PRACICON` cell 39); the hovered
item's name shows at the bottom in the large font ("Slippery Block" seen).
`SAMPLIST.TXT` names a voice for each item.
## Open questions

- `ICONS.RNC` bytes 43,648–48,120: the layout of the wooden panel pieces.
- The selected-skill animations of the bomber, builder, basher, miner,
  climber and floater (only blocker, turner and digger were observed).
- The pointer over a lemming; the
  nuke icon's animation; what the umbrella icon and the corner marks are.
- Where the rating label lettering (3×5 capitals) comes from.
- When the game shows the scenes, the intro slides at each resolution, and
  `ENDLEMMS`, `DEFLICON` and `PRACICON` (probably practice mode
  and the level-end screen). This needs observation of the running game.

**Highlight arrow (measured; play mode with the serial mouse and the
Practice "Hi-Light" and "Virtual Lemming" demos).** Clicking the panel's arrow
(cell 38) switches highlighting on: the icon turns green and spins (cells
39–44) and the lemming nearest the middle of the view gets a white down arrow
over its head (`BOMBNUMB` cell 6, a sprite in the 3D world, still, hidden by
nearer objects). A click on a lemming moves the highlight to it. Clicking a
skill gives it straight to the highlighted lemming without selecting it (the
icon plays its animation once); the highlight stays. The face then rides
along with the highlighted lemming at once; the face again leaves, the
highlight staying. Entering the lemming view with the face and a click also
highlights that lemming. Switching highlighting on deselects the skill, since
a skill button would then give it to the highlighted lemming (owner's
observation). The arrow again switches highlighting off, except in the
lemming view, where it moves on to the next lemming and rides along with it
(owner's observation; ours takes the next in release order). Highlighting also
ends when the lemming ridden with dies or leaves (about 2.7 s after). No arrow
shows over the lemming ridden with.

**Practice grid order.** The face (Virtual Lemming, level 90
"Claustrophobic") stands before the arrow (Hi-Light Lemming, level 89) in the
second row (hover labels and briefings checked).
