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
contact sheets.

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
spans columns 32–51 and rows 5–20, larger than a 16×16 face, so where
exactly a face is drawn, and which head goes with which button, is
**unverified**. The rating signs fit the board on the fourth button.

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
period was found. Only letters A–Z exist, no digits. Which frames the game
holds while a letter is shown is **unverified**.

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

Which of the green and red versions means "pressed" or "selected" is
**unverified**.

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
| `MINILEMM.GFX` | 65,536 | 64 × 32×32 | `LM3D.PAL` | Small lemmings, animated: a fat lemming, a group of lemmings shrinking into the distance, a lemming turning, a climber on a stone wall, a lemming with arms out (blocker), lemmings with a hammer, a brick bag and plank, a pickaxe, and one digging. [LF1590]: the skill icons at the bottom of the game screen. Which frames belong to which skill is **unverified**. |
| `BOMBNUMB.GFX` | 8,192 | 8 × 32×32, two per row (64 wide) | `LM3D.PAL` | Brush-stroke `1`–`5`, `?`, a white down arrow, an empty cell. [LF1590]: bomber countdown, camera number, the `?` of a builder out of bricks, the highlight cursor. |
| `COGS.GFX` | 26,136 | 3 × 88×99 | the palette in `LOADING.RNC` | Three meshing cogs (blue, green, red), 3 animation frames. With `LM3D.PAL` the colours are garbled; with the `LOADING.RNC` palette they are clean shaded blue, green and red. [LF1590] guessed the 88-pixel width. |
| `MOUSE.RNC` | 11,008 | 43 × 16×16 | `LM3D.PAL` | Mouse pointers: turn left, up, turn right, left, down, right, two diagonal turns, a bracket frame, a cross-hair, a target, then a white arrow in 32 directions, turning clockwise from straight up in steps of 11.25°. |
| `DEFLICON.RNC` | 32,768 | 32 × 32×32 | `LM3D.PAL` | A brown wedge-shaped block turning through 32 angles. |
| `PRACICON.RNC` | 57,344 | 56 × 32×32 | `LM3D.PAL` | Object pictures with their animations: a red pad (4), a stone table (2), one-way arrows (4), a lemming on a rope slide (5), a white disc (4), a spring (7), soil (8), a lemming waving (5), a green tick (1), an "EXIT" sign turning round (16). |
| `ENDLEMMS.RNC` | 5,632 | 8 × 22×32 | `LM3D.PAL` | A lemming facing the viewer, raising its arms and turning round. |
| `WINDER.RNC` | 32,768 | 8 × 64×64 | `LM3D.PAL` | A lemming turning a large wheel, side view. |

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

## Open questions

- `ICONS.RNC` bytes 43,648–48,120: the layout of the wooden panel pieces.
- Where the menu faces sit in the button holes, and which head belongs to
  which button.
- Which `MINILEMM.GFX` frames make up each skill's button animation, and in
  what order the panel uses the green and red icon versions.
- When the game shows the scenes, the intro slides at each resolution, and
  `WINDER`, `ENDLEMMS`, `DEFLICON` and `PRACICON` (probably practice mode
  and the level-end screen). This needs observation of the running game.
