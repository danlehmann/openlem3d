//! Rob Northen Computing "ProPack" method 1 decompression. See
//! `docs/spec/rnc.md`.

use crate::Error;

/// Length of the RNC header.
pub const HEADER_LEN: usize = 18;

/// The fixed fields at the start of an RNC-packed file.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// Compression method (only 1 is supported).
    pub method: u8,
    pub unpacked_len: u32,
    pub packed_len: u32,
    /// CRC-16 of the unpacked data.
    pub unpacked_crc: u16,
    /// CRC-16 of the packed data (the bytes following the header).
    pub packed_crc: u16,
    /// Bytes by which in-place decompression may overrun the packed data.
    pub leeway: u8,
    /// Number of Huffman-coded blocks in the stream.
    pub blocks: u8,
}

/// Returns true if `data` starts with an RNC signature.
pub fn is_packed(data: &[u8]) -> bool {
    data.len() >= HEADER_LEN && &data[..3] == b"RNC"
}

pub fn parse_header(data: &[u8]) -> Result<Header, Error> {
    if !is_packed(data) {
        return Err(Error::Format("missing RNC signature".into()));
    }
    let be32 = |o: usize| u32::from_be_bytes(data[o..o + 4].try_into().unwrap());
    let be16 = |o: usize| u16::from_be_bytes(data[o..o + 2].try_into().unwrap());
    Ok(Header {
        method: data[3],
        unpacked_len: be32(4),
        packed_len: be32(8),
        unpacked_crc: be16(12),
        packed_crc: be16(14),
        leeway: data[16],
        blocks: data[17],
    })
}

/// CRC-16 with the reflected polynomial 0xA001 and initial value 0.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Bit reader over the packed stream. Bits are consumed LSB-first from
/// little-endian 16-bit words; `buf` always holds the word at `pos` in its
/// top 16 valid bits, so literal runs can be read byte-aligned from `pos`.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    count: u32,
}

impl<'a> Bits<'a> {
    fn word(&self, at: usize) -> u32 {
        let b = |i: usize| self.data.get(i).copied().unwrap_or(0) as u32;
        b(at) | b(at + 1) << 8
    }

    fn new(data: &'a [u8]) -> Self {
        let mut s = Bits {
            data,
            pos: 0,
            buf: 0,
            count: 16,
        };
        s.buf = s.word(0);
        s
    }

    fn peek(&self, n: u32) -> u32 {
        self.buf & ((1u32 << n) - 1)
    }

    fn advance(&mut self, n: u32) {
        self.buf >>= n;
        self.count -= n;
        if self.count < 16 {
            self.pos += 2;
            self.buf |= self.word(self.pos) << self.count;
            self.count += 16;
        }
    }

    fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.advance(n);
        v
    }

    /// Replaces the look-ahead word with the word at the (moved) `pos`.
    fn refill(&mut self) {
        self.count -= 16;
        self.buf &= (1u32 << self.count) - 1;
        self.buf |= self.word(self.pos) << self.count;
        self.count += 16;
    }
}

/// A canonical Huffman code; entries are `(code, length, symbol)` with codes
/// bit-reversed to match the LSB-first stream.
struct Huffman(Vec<(u32, u32, u32)>);

impl Huffman {
    fn read(bits: &mut Bits) -> Self {
        let n = bits.read(5) as usize;
        let lens: Vec<u32> = (0..n).map(|_| bits.read(4)).collect();
        let mut table = Vec::new();
        let mut code = 0u32;
        for len in 1..=16 {
            for (sym, _) in lens.iter().enumerate().filter(|(_, l)| **l == len) {
                let rev = code.reverse_bits() >> (32 - len);
                table.push((rev, len, sym as u32));
                code += 1;
            }
            code <<= 1;
        }
        Huffman(table)
    }

    /// Decodes one value: symbol 0 and 1 are literal; symbol `k >= 2` is
    /// followed by `k - 1` extra bits giving `2^(k-1) + extra`.
    fn decode(&self, bits: &mut Bits) -> Result<u32, Error> {
        let &(_, len, sym) = self
            .0
            .iter()
            .find(|(code, len, _)| bits.peek(*len) == *code)
            .ok_or_else(|| Error::Format("RNC: invalid Huffman code".into()))?;
        bits.advance(len);
        if sym < 2 {
            return Ok(sym);
        }
        Ok((1 << (sym - 1)) | bits.read(sym - 1))
    }
}

/// Decompresses an RNC method-1 file (header included), verifying both CRCs.
pub fn unpack(data: &[u8]) -> Result<Vec<u8>, Error> {
    let h = parse_header(data)?;
    if h.method != 1 {
        return Err(Error::Format(format!(
            "RNC method {} unsupported",
            h.method
        )));
    }
    let packed = data
        .get(HEADER_LEN..HEADER_LEN + h.packed_len as usize)
        .ok_or_else(|| Error::Format("RNC: truncated".into()))?;
    if crc16(packed) != h.packed_crc {
        return Err(Error::Format("RNC: packed CRC mismatch".into()));
    }
    let out_len = h.unpacked_len as usize;
    let mut out = Vec::with_capacity(out_len);
    let mut bits = Bits::new(packed);
    bits.advance(2); // lock and key flags
    let corrupt = || Error::Format("RNC: corrupt stream".into());
    while out.len() < out_len {
        let raw = Huffman::read(&mut bits);
        let dist = Huffman::read(&mut bits);
        let len = Huffman::read(&mut bits);
        let mut chunks = bits.read(16);
        loop {
            let n = raw.decode(&mut bits)? as usize;
            if n > 0 {
                let lit = packed.get(bits.pos..bits.pos + n).ok_or_else(corrupt)?;
                out.extend_from_slice(lit);
                bits.pos += n;
                bits.refill();
            }
            chunks = chunks.wrapping_sub(1);
            if chunks == 0 || chunks > 0xFFFF {
                break;
            }
            let back = dist.decode(&mut bits)? as usize + 1;
            let count = len.decode(&mut bits)? as usize + 2;
            let start = out.len().checked_sub(back).ok_or_else(corrupt)?;
            for i in 0..count {
                out.push(out[start + i]);
            }
        }
    }
    if out.len() != out_len {
        return Err(Error::Format(format!(
            "RNC: unpacked {} bytes, expected {out_len}",
            out.len()
        )));
    }
    if crc16(&out) != h.unpacked_crc {
        return Err(Error::Format("RNC: unpacked CRC mismatch".into()));
    }
    Ok(out)
}

/// Returns `data` decompressed if it is RNC-packed, or unchanged otherwise.
pub fn unpack_if_packed(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    if is_packed(&data) {
        unpack(&data)
    } else {
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_check_value() {
        // CRC-16/ARC check value.
        assert_eq!(crc16(b"123456789"), 0xBB3D);
    }
}
