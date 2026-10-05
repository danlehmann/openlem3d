# References and credits

openlem3d builds on reverse-engineering work published by the Lemmings
community. This page lists every external source we use. Spec documents cite
these sources by their tag, for example **[L3DEdit]**.

| Tag | Source | Authors | Covers |
|---|---|---|---|
| **[LF1590]** | Lemmings Forums, *"Lemmings 3D file formats"* thread — <https://www.lemmingsforums.net/index.php?topic=1590.0> | Pooty (original documentation, 2013), GuyPerfect (MHC sprite format), namida, ccexplore | `LEVEL`/`BLK` layout, GFX file widths, MHC lemming sprites, palette |
| **[L3DEdit]** | L3DEdit notes — <https://bitbucket.org/namida42/l3dedit/src/master/notes/> | namida (incorporating Pooty's notes; save checksum cracked by ccexplore) | Detailed `LEVEL`, `BLK`, graphics, object placement, save files, style indexes |
| **[Owner]** | Reverse-engineering notes supplied by the project owner (partly drawn from the sources above, partly found by an earlier AI agent) | project owner | `LEVEL`/`BLK`/`TEXTURE` layout, cameras, RNC header |
| **[RNC]** | Public descriptions of Rob Northen Computing's *ProPack* method 1 compression (for example, Simon Tatham's public-domain `dernc`) | Rob Northen (format), Simon Tatham (decoder description) | RNC decompression |
