# RNC ProPack method 1

Sources: [RNC], [L3DEdit] (which notes that the game also accepts uncompressed
files), [Owner].

**Status:** verified. Our decoder unpacks every RNC file on the disc, 150 in
all, and the packed and unpacked CRCs match for each.

## Header (18 bytes, big-endian)

| Offset | Size | Field |
|---|---|---|
| 0 | 3 | Signature `RNC` |
| 3 | 1 | Method; always 1 on this disc |
| 4 | 4 | Unpacked length |
| 8 | 4 | Packed length (= file length − 18 on this disc) |
| 12 | 2 | CRC-16 of the unpacked data |
| 14 | 2 | CRC-16 of the packed data |
| 16 | 1 | Leeway: how far in-place unpacking may overrun; always 0 here |
| 17 | 1 | Number of blocks in the stream |

**Wrong:** [Owner] describes bytes 16–17 as a single 16-bit "overlap size".
They are two separate one-byte fields. The block count varies between files
and matches the number of blocks the decoder reads.

The CRC is CRC-16/ARC: reflected polynomial 0xA001, initial value 0, no final
XOR.

## Bitstream

Bits are taken least-significant-bit first, from little-endian 16-bit words.

The reader keeps one 16-bit word of look-ahead. When a literal run occurs, its
bytes are copied straight from the byte position of that look-ahead word, and
the look-ahead word is then reloaded from the byte position after the run.

Decoding proceeds as follows:

1. Skip 2 bits (flag bits; they are always 0 here).
2. Until the output is complete, decode one block at a time:
   1. Read three Huffman tables in order: *literal-length*, *distance* and
      *match-length*. Each table is stored as a 5-bit symbol count `n`,
      followed by `n` 4-bit code lengths (0 means the symbol is unused).
      Codes are canonical, assigned in order of increasing code length and
      then symbol number. They are stored bit-reversed, to match the LSB-first
      stream.
   2. Read a 16-bit command count `c`.
   3. Repeat `c` times:
      1. Decode a literal length `L` and copy `L` raw bytes from the byte
         position (this can be 0).
      2. Unless this is the last of the `c` commands, decode a distance `d`
         and a length `m`. Copy `m + 2` bytes from `d + 1` bytes back in the
         output.

Decoding a value from a table works like this: symbols 0 and 1 stand for
themselves. A symbol `k ≥ 2` is followed by `k − 1` extra bits `e`, and the
value is `2^(k−1) + e`.

The implementation is in `crates/l3d-formats/src/rnc.rs`.
