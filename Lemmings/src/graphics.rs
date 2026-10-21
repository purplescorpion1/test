// graphics.rs — Amiga 4-bitplane planar → 32-bit RGBA converter
// Language: Rust, file: src/graphics.rs

use anyhow::{bail, Result};

// Amiga OCS hardware: 32-colour palette, 4 bitplanes
pub const COLOURS:   usize = 32;
pub const BITPLANES: usize = 4;

// A decoded palette: 32 RGBA entries
pub type Palette = [[u8; 4]; COLOURS];

// Ground file layout (Ground1–5, Objects1–5):
//   Ground:  terrain tiles + palette + collision mask
//   Objects: object sprites + palette
//
// Ground?dat structure (from Lemmings RE docs):
//   16 terrain sprites × 40×8 pixels, 4 bitplanes = 16 × (4 × 40 bytes) = 2560 bytes
//   Then: 8 object frames × (variable size)
//   Then: palette at known offset (0x0560)
//   Then: collision mask at 0x0660 (1-bit/pixel, same dim as sprites)
//
// Objects?dat structure:
//   Object sprites, palette at end.

pub const TERRAIN_TILE_W: usize = 8;
pub const TERRAIN_TILE_H: usize = 8; // displayed; raw plane height 8 rows
pub const TERRAIN_TILES:  usize = 64;

// One decoded tile: w×h RGBA pixels
#[derive(Debug, Clone)]
pub struct Tile {
    pub w:     usize,
    pub h:     usize,
    pub rgba:  Vec<u8>,   // w*h*4
    pub solid: Vec<bool>, // w*h collision mask
}

impl Tile {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            rgba:  vec![0u8; w * h * 4],
            solid: vec![false; w * h],
        }
    }
}

// A decoded object animation frame
#[derive(Debug, Clone)]
pub struct Frame {
    pub w:    usize,
    pub h:    usize,
    pub rgba: Vec<u8>,
}

// Full graphics set for one style
pub struct StyleGraphics {
    pub terrain_tiles: Vec<Tile>,     // up to 64
    pub object_frames: Vec<Vec<Frame>>, // per-object: vec of animation frames
    pub palette:       Palette,
}

/// Decode a single 4-bitplane Amiga row into colour indices.
/// `planes` is 4 pointers into the raw data at the correct byte offsets.
/// Width is in pixels (one byte covers 8 pixels per plane).
fn decode_row(planes: [&[u8]; BITPLANES], pixel_w: usize, out: &mut [u8]) {
    let byte_w = (pixel_w + 7) / 8;
    for byte_i in 0..byte_w {
        for bit in 0..8 {
            let pixel = byte_i * 8 + bit;
            if pixel >= pixel_w { break; }
            let shift = 7 - bit;
            let mut index = 0u8;
            for p in 0..BITPLANES {
                if byte_i < planes[p].len() {
                    index |= ((planes[p][byte_i] >> shift) & 1) << p;
                }
            }
            out[pixel] = index;
        }
    }
}

/// Convert Amiga 12-bit colour word (0x0RGB) → RGBA [r,g,b,255].
/// Amiga extends 4-bit channels by replicating: r4 → r8 = (r4 << 4) | r4.
fn amiga_colour(word: u16) -> [u8; 4] {
    let r4 = ((word >> 8) & 0xF) as u8;
    let g4 = ((word >> 4) & 0xF) as u8;
    let b4 = (word & 0xF) as u8;
    [
        (r4 << 4) | r4,
        (g4 << 4) | g4,
        (b4 << 4) | b4,
        255,
    ]
}

/// Parse a 32-entry Amiga palette from raw bytes at `offset`.
/// Each entry is 2 bytes big-endian: 0x0RGB.
pub fn parse_palette(data: &[u8], offset: usize) -> Result<Palette> {
    if offset + COLOURS * 2 > data.len() {
        bail!(
            "Palette offset 0x{:x} out of range (file size {})",
            offset,
            data.len()
        );
    }
    let mut pal = [[0u8; 4]; COLOURS];
    for i in 0..COLOURS {
        let hi = data[offset + i * 2] as u16;
        let lo = data[offset + i * 2 + 1] as u16;
        pal[i] = amiga_colour((hi << 8) | lo);
    }
    // Colour 0 is transparent for sprites
    pal[0][3] = 0;
    Ok(pal)
}

/// Decode terrain tiles from a Ground dat file.
/// The terrain tile sheet starts at byte 0 and contains:
///   TERRAIN_TILES × 4-bitplane rows, each tile is 8px wide × 8px tall.
///   Plane stride = ceil(8/8) = 1 byte/row/plane.
///   Total per tile = 4 planes × 8 rows × 1 byte = 32 bytes.
pub fn decode_terrain_tiles(
    data:    &[u8],
    palette: &Palette,
    mask_offset: usize,
) -> Result<Vec<Tile>> {
    const TW: usize = TERRAIN_TILE_W;
    const TH: usize = TERRAIN_TILE_H;
    const BYTES_PER_ROW_PER_PLANE: usize = 1; // ceil(8/8)
    const BYTES_PER_TILE: usize = TH * BITPLANES * BYTES_PER_ROW_PER_PLANE; // 32

    let mut tiles = Vec::with_capacity(TERRAIN_TILES);

    for t in 0..TERRAIN_TILES {
        let tile_base = t * BYTES_PER_TILE;
        if tile_base + BYTES_PER_TILE > data.len() {
            break; // fewer tiles in this ground file
        }

        let mut tile = Tile::new(TW, TH);

        for row in 0..TH {
            let row_base = tile_base + row * BITPLANES * BYTES_PER_ROW_PER_PLANE;
            let planes = [
                &data[row_base + 0 * BYTES_PER_ROW_PER_PLANE
                    ..row_base + 0 * BYTES_PER_ROW_PER_PLANE + BYTES_PER_ROW_PER_PLANE],
                &data[row_base + 1 * BYTES_PER_ROW_PER_PLANE
                    ..row_base + 1 * BYTES_PER_ROW_PER_PLANE + BYTES_PER_ROW_PER_PLANE],
                &data[row_base + 2 * BYTES_PER_ROW_PER_PLANE
                    ..row_base + 2 * BYTES_PER_ROW_PER_PLANE + BYTES_PER_ROW_PER_PLANE],
                &data[row_base + 3 * BYTES_PER_ROW_PER_PLANE
                    ..row_base + 3 * BYTES_PER_ROW_PER_PLANE + BYTES_PER_ROW_PER_PLANE],
            ];

            let mut row_indices = [0u8; TW];
            decode_row(planes, TW, &mut row_indices);

            for col in 0..TW {
                let pixel = row * TW + col;
                let idx   = row_indices[col] as usize;
                let rgba  = palette[idx];
                tile.rgba[pixel * 4 + 0] = rgba[0];
                tile.rgba[pixel * 4 + 1] = rgba[1];
                tile.rgba[pixel * 4 + 2] = rgba[2];
                tile.rgba[pixel * 4 + 3] = rgba[3];
            }
        }

        // Collision mask: 1-bit per pixel, laid out after tile pixels
        if mask_offset + t * ((TW + 7) / 8) * TH <= data.len() {
            let mask_base = mask_offset + t * ((TW + 7) / 8) * TH;
            for row in 0..TH {
                for col in 0..TW {
                    let byte_off = mask_base + row * ((TW + 7) / 8) + col / 8;
                    let bit      = 7 - (col % 8);
                    if byte_off < data.len() {
                        tile.solid[row * TW + col] = (data[byte_off] >> bit) & 1 == 1;
                    }
                }
            }
        } else {
            // Fallback: pixel is solid if colour index != 0
            // (re-derive from rgba alpha)
            for i in 0..(TW * TH) {
                tile.solid[i] = tile.rgba[i * 4 + 3] != 0;
            }
        }

        tiles.push(tile);
    }

    Ok(tiles)
}

/// Decode a lemming/object sprite sheet (16 frames, each sprite_w × sprite_h).
/// `pixel_w` and `pixel_h` vary by sprite type.
/// The sprite sheet interleaves rows: plane0_row0 plane1_row0 plane2_row0 plane3_row0
///                                   plane0_row1 ...
pub fn decode_sprite_sheet(
    data:     &[u8],
    offset:   usize,
    frames:   usize,
    sprite_w: usize,
    sprite_h: usize,
    palette:  &Palette,
) -> Result<Vec<Frame>> {
    let bytes_per_row = (sprite_w + 7) / 8;
    let bytes_per_frame = sprite_h * BITPLANES * bytes_per_row;
    let mut out = Vec::with_capacity(frames);

    for f in 0..frames {
        let frame_base = offset + f * bytes_per_frame;
        if frame_base + bytes_per_frame > data.len() {
            break;
        }

        let mut rgba = vec![0u8; sprite_w * sprite_h * 4];

        for row in 0..sprite_h {
            let row_base = frame_base + row * BITPLANES * bytes_per_row;
            let planes = [
                &data[row_base..row_base + bytes_per_row],
                &data[row_base + bytes_per_row..row_base + 2 * bytes_per_row],
                &data[row_base + 2 * bytes_per_row..row_base + 3 * bytes_per_row],
                &data[row_base + 3 * bytes_per_row..row_base + 4 * bytes_per_row],
            ];

            let mut row_indices = vec![0u8; sprite_w];
            decode_row(planes, sprite_w, &mut row_indices);

            for col in 0..sprite_w {
                let pixel  = row * sprite_w + col;
                let colour = palette[row_indices[col] as usize];
                rgba[pixel * 4..pixel * 4 + 4].copy_from_slice(&colour);
            }
        }

        out.push(Frame { w: sprite_w, h: sprite_h, rgba });
    }

    Ok(out)
}

// Known ground file layout offsets (from Lemmings reverse engineering)
pub const GROUND_PALETTE_OFFSET:  usize = 0x0560;
pub const GROUND_MASK_OFFSET:     usize = 0x0660;
pub const GROUND_OBJECT_OFFSET:   usize = 0x0800; // objects start here in ground file
// Lemming sprite file ("main") layout
pub const LEMMING_SPRITE_W:    usize = 16;
pub const LEMMING_SPRITE_H:    usize = 10;
pub const LEMMING_FRAME_COUNT: usize = 8;
