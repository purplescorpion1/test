// decompress.rs — Rob Northen Compression (RNC) bitstream decompressor
// Used by DMA Design for all Lemmings assets: levels, graphics, audio.
// Language: Rust, file: src/decompress.rs

use anyhow::{bail, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

const RNC_MAGIC: u32 = 0x524E4331; // "RNC1"

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    // If no RNC magic, return as-is (uncompressed file)
    if data.len() < 4 {
        return Ok(data.to_vec());
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if magic != RNC_MAGIC {
        return Ok(data.to_vec());
    }
    rnc1_unpack(data)
}

// RNC1 format header (18 bytes):
//   u32 magic        "RNC1"
//   u32 unpacked_len
//   u32 packed_len
//   u16 unpacked_crc
//   u16 packed_crc
//   u8  leeway        (ignored)
//   u8  num_chunks
fn rnc1_unpack(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 18 {
        bail!("RNC header truncated");
    }
    let mut cur = Cursor::new(data);
    cur.read_u32::<BigEndian>()?; // magic
    let unpacked_len = cur.read_u32::<BigEndian>()? as usize;
    let packed_len   = cur.read_u32::<BigEndian>()? as usize;
    cur.read_u16::<BigEndian>()?; // unpacked_crc
    cur.read_u16::<BigEndian>()?; // packed_crc
    cur.read_u8()?;               // leeway
    let _num_chunks = cur.read_u8()?;

    let packed_start = 18usize;
    let packed_end   = packed_start + packed_len;
    if packed_end > data.len() {
        bail!("RNC packed data truncated: need {} have {}", packed_end, data.len());
    }

    let packed = &data[packed_start..packed_end];
    let mut out = Vec::with_capacity(unpacked_len);

    let mut bs = BitStream::new(packed);
    // Prime the bit buffer
    bs.prime();

    while out.len() < unpacked_len {
        // Each chunk starts with three Huffman tables: raw, dist, len
        let raw_table  = HuffTable::read(&mut bs)?;
        let dist_table = HuffTable::read(&mut bs)?;
        let len_table  = HuffTable::read(&mut bs)?;
        let count      = bs.read_bits(16) as usize;

        for _ in 0..count {
            // 0-bit: literal; 1-bit: back-reference
            if bs.read_bit() == 0 {
                let sym = raw_table.decode(&mut bs) as u8;
                out.push(sym);
            } else {
                let dist = dist_table.decode(&mut bs) as usize + 1;
                let len  = len_table.decode(&mut bs) as usize + 2;
                let base = out.len();
                if base < dist {
                    bail!("RNC back-reference before start: base={} dist={}", base, dist);
                }
                for i in 0..len {
                    let b = out[base - dist + i];
                    out.push(b);
                }
            }
        }
    }

    if out.len() != unpacked_len {
        bail!("RNC size mismatch: got {} expected {}", out.len(), unpacked_len);
    }
    Ok(out)
}

// ── Huffman table ────────────────────────────────────────────────────────────

struct HuffTable {
    // Parallel arrays: code, mask, value
    entries: Vec<(u32, u32, u16)>,
}

impl HuffTable {
    fn read(bs: &mut BitStream) -> Result<Self> {
        let num_syms = bs.read_bits(5) as usize;
        let mut entries = Vec::with_capacity(num_syms);

        if num_syms == 0 {
            return Ok(Self { entries });
        }

        for _ in 0..num_syms {
            let code_len = bs.read_bits(4) as u32;
            let sym_val  = bs.read_bits(code_len as usize) as u16;
            if code_len == 0 {
                entries.push((0, 0, sym_val));
            } else {
                let mask = (1u32 << code_len) - 1;
                entries.push((sym_val as u32, mask, sym_val));
            }
        }
        Ok(Self { entries })
    }

    fn decode(&self, bs: &mut BitStream) -> u16 {
        if self.entries.len() == 1 {
            return self.entries[0].2;
        }
        // Peek bits and match
        let mut code = 0u32;
        for bit_pos in 0..16u32 {
            let b = bs.read_bit();
            code |= (b as u32) << bit_pos;
            let mask = (1u32 << (bit_pos + 1)) - 1;
            for &(c, m, v) in &self.entries {
                if m == mask && (c & mask) == (code & mask) {
                    return v;
                }
            }
        }
        // fallback: return first symbol
        self.entries[0].2
    }
}

// ── Bit-stream (LSB-first) ───────────────────────────────────────────────────

struct BitStream<'a> {
    data:    &'a [u8],
    pos:     usize,
    buf:     u32,
    bits_in: u32,
}

impl<'a> BitStream<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, buf: 0, bits_in: 0 }
    }

    fn prime(&mut self) {
        // Load two bytes into buffer
        self.buf     = self.next_byte() as u32 | ((self.next_byte() as u32) << 8);
        self.bits_in = 16;
    }

    fn next_byte(&mut self) -> u8 {
        if self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            b
        } else {
            0
        }
    }

    fn refill(&mut self) {
        while self.bits_in <= 24 {
            self.buf     |= (self.next_byte() as u32) << self.bits_in;
            self.bits_in += 8;
        }
    }

    fn read_bit(&mut self) -> u8 {
        if self.bits_in == 0 {
            self.refill();
        }
        let b = (self.buf & 1) as u8;
        self.buf     >>= 1;
        self.bits_in  -= 1;
        b
    }

    fn read_bits(&mut self, n: usize) -> u32 {
        if n == 0 { return 0; }
        self.refill();
        let v = self.buf & ((1u32 << n) - 1);
        self.buf     >>= n;
        self.bits_in  -= n as u32;
        v
    }
}
