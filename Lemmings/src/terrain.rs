// terrain.rs — Pixel-level terrain bitmap with destruction and collision masks
// Language: Rust, file: src/terrain.rs

use anyhow::Result;
use crate::level::{Level, TerrainTile as TileRef};
use crate::graphics::{StyleGraphics, Tile};

// Lemmings level playfield dimensions (Amiga original)
pub const LEVEL_W: usize = 1600;
pub const LEVEL_H: usize = 160;

// Each pixel is RGBA (4 bytes) in the composited playfield surface
pub const PLAYFIELD_BYTES: usize = LEVEL_W * LEVEL_H * 4;

/// The terrain state: a flat RGBA pixel buffer for rendering
/// plus a 1-bit-per-pixel solid mask for collision,
/// and a 1-bit-per-pixel steel mask (undestruct­ible).
pub struct Terrain {
    pub pixels: Vec<u8>,       // LEVEL_W × LEVEL_H × 4 RGBA, origin top-left
    pub solid:  Vec<bool>,     // LEVEL_W × LEVEL_H
    pub steel:  Vec<bool>,     // LEVEL_W × LEVEL_H
    pub water:  Vec<bool>,     // LEVEL_W × LEVEL_H (from level objects)
    pub exits:  Vec<(i32,i32,u8,u8)>, // (x,y,w,h) for each exit hitbox
    pub traps:  Vec<(i32,i32,u8,u8)>, // trap hitboxes
}

impl Terrain {
    /// Build the terrain from a parsed level + decoded graphics.
    pub fn build(level: &Level, gfx: &StyleGraphics) -> Result<Self> {
        let mut pixels = vec![0u8; PLAYFIELD_BYTES];
        let mut solid  = vec![false; LEVEL_W * LEVEL_H];
        let mut steel  = vec![false; LEVEL_W * LEVEL_H];
        let water      = vec![false; LEVEL_W * LEVEL_H];
        let mut exits  = Vec::new();
        let mut traps  = Vec::new();

        // ── Blit terrain tiles ───────────────────────────────────────────────
        for tile_ref in &level.terrain {
            let tile_idx = tile_ref.tile_id as usize;
            if tile_idx >= gfx.terrain_tiles.len() { continue; }
            let tile = &gfx.terrain_tiles[tile_idx];

            blit_tile(
                &mut pixels,
                &mut solid,
                tile,
                tile_ref,
                LEVEL_W,
                LEVEL_H,
            );
        }

        // ── Steel areas ──────────────────────────────────────────────────────
        for area in &level.steel {
            let x0 = area.x as i32;
            let y0 = area.y as i32;
            for dy in 0..area.h as i32 {
                for dx in 0..area.w as i32 {
                    let px = x0 + dx;
                    let py = y0 + dy;
                    if in_bounds(px, py) {
                        steel[py as usize * LEVEL_W + px as usize] = true;
                    }
                }
            }
        }

        // ── Level objects: mark exits, traps, water ──────────────────────────
        for obj in &level.objects {
            use crate::level::ObjectKind;
            match obj.kind {
                ObjectKind::Exit     => exits.push((obj.x as i32, obj.y as i32, 16, 16)),
                ObjectKind::Trap     => traps.push((obj.x as i32, obj.y as i32, 16, 16)),
                ObjectKind::Trapdoor => {} // handled by game spawner
            }
        }

        Ok(Self { pixels, solid, steel, water, exits, traps })
    }

    // ── Query ────────────────────────────────────────────────────────────────

    #[inline]
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        if !in_bounds(x, y) { return false; }
        self.solid[y as usize * LEVEL_W + x as usize]
    }

    #[inline]
    pub fn is_steel(&self, x: i32, y: i32) -> bool {
        if !in_bounds(x, y) { return false; }
        self.steel[y as usize * LEVEL_W + x as usize]
    }

    #[inline]
    pub fn is_water(&self, x: i32, y: i32) -> bool {
        if !in_bounds(x, y) { return false; }
        self.water[y as usize * LEVEL_W + x as usize]
    }

    pub fn is_exit(&self, x: i32, y: i32) -> bool {
        self.exits.iter().any(|&(ex, ey, ew, eh)| {
            x >= ex && x < ex + ew as i32 && y >= ey && y < ey + eh as i32
        })
    }

    pub fn is_trap(&self, x: i32, y: i32) -> bool {
        self.traps.iter().any(|&(tx, ty, tw, th)| {
            x >= tx && x < tx + tw as i32 && y >= ty && y < ty + th as i32
        })
    }

    // ── Mutate (destruction) ─────────────────────────────────────────────────

    pub fn set_solid(&mut self, x: i32, y: i32, v: bool) {
        if !in_bounds(x, y) { return; }
        if self.steel[y as usize * LEVEL_W + x as usize] { return; }
        let idx = y as usize * LEVEL_W + x as usize;
        self.solid[idx] = v;
        // Clear pixel if erased
        if !v {
            let px = idx * 4;
            self.pixels[px]     = 0;
            self.pixels[px + 1] = 0;
            self.pixels[px + 2] = 0;
            self.pixels[px + 3] = 0;
        }
    }

    /// Set a terrain pixel colour (used when building)
    pub fn set_pixel(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        if !in_bounds(x, y) { return; }
        let idx = y as usize * LEVEL_W + x as usize;
        self.solid[idx] = rgba[3] != 0;
        let px = idx * 4;
        self.pixels[px..px + 4].copy_from_slice(&rgba);
    }

    /// Return a slice of the pixel row at `y` for rendering.
    pub fn row_slice(&self, y: usize, x_start: usize, w: usize) -> &[u8] {
        let base = (y * LEVEL_W + x_start) * 4;
        let end  = base + w * 4;
        &self.pixels[base..end.min(self.pixels.len())]
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

#[inline]
fn in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && (x as usize) < LEVEL_W && y >= 0 && (y as usize) < LEVEL_H
}

fn blit_tile(
    pixels:   &mut Vec<u8>,
    solid:    &mut Vec<bool>,
    tile:     &Tile,
    tile_ref: &TileRef,
    level_w:  usize,
    level_h:  usize,
) {
    let dst_x0 = tile_ref.x as i32;
    let dst_y0 = tile_ref.y as i32 + 16; // Amiga level has 16px header band

    let flip     = tile_ref.flip_x();
    let erase    = tile_ref.erase();
    let no_over  = tile_ref.no_overwrite();

    for ty in 0..tile.h as i32 {
        for tx in 0..tile.w as i32 {
            let dst_x = dst_x0 + tx;
            let dst_y = dst_y0 + ty;
            if dst_x < 0 || dst_x >= level_w as i32
            || dst_y < 0 || dst_y >= level_h as i32 {
                continue;
            }

            let src_tx = if flip { tile.w as i32 - 1 - tx } else { tx };
            let src_idx = (ty as usize * tile.w + src_tx as usize) * 4;
            if src_idx + 3 >= tile.rgba.len() { continue; }

            let alpha = tile.rgba[src_idx + 3];
            let dst_idx = (dst_y as usize * level_w + dst_x as usize) * 4;

            if erase {
                // Erase mode: make transparent regardless
                pixels[dst_idx]     = 0;
                pixels[dst_idx + 1] = 0;
                pixels[dst_idx + 2] = 0;
                pixels[dst_idx + 3] = 0;
                solid[dst_y as usize * level_w + dst_x as usize] = false;
            } else if no_over {
                // No-overwrite: only draw where nothing exists
                if pixels[dst_idx + 3] == 0 && alpha != 0 {
                    pixels[dst_idx..dst_idx + 4]
                        .copy_from_slice(&tile.rgba[src_idx..src_idx + 4]);
                    solid[dst_y as usize * level_w + dst_x as usize] = true;
                }
            } else if alpha != 0 {
                pixels[dst_idx..dst_idx + 4]
                    .copy_from_slice(&tile.rgba[src_idx..src_idx + 4]);
                solid[dst_y as usize * level_w + dst_x as usize] = true;
            }
        }
    }
}
