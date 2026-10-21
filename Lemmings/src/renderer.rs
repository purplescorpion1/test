// renderer.rs — SDL2 rendering: terrain, lemmings, UI panel
// Language: Rust, file: src/renderer.rs

use anyhow::Result;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator, BlendMode};
use sdl2::video::{Window, WindowContext};

use crate::game::GamePhase;
use crate::graphics::{StyleGraphics, Frame, LEMMING_SPRITE_W, LEMMING_SPRITE_H};
use crate::lemming::{Lemming, Direction};
use crate::terrain::{Terrain, LEVEL_W, LEVEL_H};
use crate::level::Skill;

pub const WINDOW_W: u32 = 800;
pub const WINDOW_H: u32 = 400;
pub const PANEL_H:  u32 = 56;
pub const PANEL_W:  u32 = 0;          // skill panel is at bottom, not right
pub const VIEW_W:   u32 = WINDOW_W;
pub const VIEW_H:   u32 = WINDOW_H - PANEL_H;  // 344px of level visible

// Skill panel layout: 8 buttons + rate/time/nuke strip
pub const BUTTON_W: u32 = 32;
pub const BUTTON_H: u32 = PANEL_H;
pub const BUTTON_COUNT: usize = 8;

pub struct Renderer;

impl Renderer {
    pub fn draw(
        canvas:     &mut Canvas<Window>,
        tc:         &TextureCreator<WindowContext>,
        terrain:    &Terrain,
        gfx:        &StyleGraphics,
        lemmings:   &[Lemming],
        cam_x:      i32,
        cam_y:      i32,
        saved:      u32,
        needed:     u16,
        total:      u16,
        time_s:     u32,
        rate:       u32,
        skills:     [u16; 8],
        selected:   Option<Skill>,
        phase:      GamePhase,
    ) -> Result<()> {
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();

        // ── Terrain texture ──────────────────────────────────────────────────
        // Build a streaming texture from the visible strip of the terrain bitmap.
        let view_x_start = cam_x.max(0) as usize;
        let view_w       = (VIEW_W as usize).min(LEVEL_W - view_x_start);
        let view_h       = VIEW_H as usize;
        let y_start      = cam_y.max(0) as usize;

        let mut terrain_tex = tc
            .create_texture_streaming(PixelFormatEnum::RGBA8888, view_w as u32, view_h as u32)
            .map_err(|e| anyhow::anyhow!("Terrain texture: {}", e))?;

        terrain_tex.set_blend_mode(BlendMode::None);
        terrain_tex.with_lock(None, |pixels, pitch| {
            for row in 0..view_h {
                let level_y = y_start + row;
                if level_y >= LEVEL_H { break; }
                let src = terrain.row_slice(level_y, view_x_start, view_w);
                let dst_base = row * pitch;
                // RGBA8888 in SDL2 on little-endian is ABGR in memory — swap to match
                for col in 0..view_w {
                    let si = col * 4;
                    let di = dst_base + col * 4;
                    if si + 3 < src.len() && di + 3 < pixels.len() {
                        // SDL RGBA8888 (big-endian component order): R G B A
                        pixels[di]     = src[si + 3]; // A
                        pixels[di + 1] = src[si + 2]; // B
                        pixels[di + 2] = src[si + 1]; // G
                        pixels[di + 3] = src[si];     // R
                    }
                }
            }
        }).map_err(|e| anyhow::anyhow!("Terrain lock: {}", e))?;

        canvas.copy(&terrain_tex, None, Rect::new(0, 0, view_w as u32, view_h as u32))
            .map_err(|e| anyhow::anyhow!("Terrain copy: {}", e))?;

        // ── Lemmings ─────────────────────────────────────────────────────────
        if let Some(frames) = gfx.object_frames.first() {
            Self::draw_lemmings(canvas, tc, lemmings, frames, cam_x, cam_y, view_h as i32)?;
        }

        // ── Skill panel ───────────────────────────────────────────────────────
        Self::draw_panel(canvas, skills, selected, saved, needed, total, time_s, rate)?;

        // ── Overlays ─────────────────────────────────────────────────────────
        match phase {
            GamePhase::Paused => {
                Self::draw_overlay(canvas, "PAUSED", Color::RGBA(0,0,0,160))?;
            }
            GamePhase::LevelComplete => {
                Self::draw_overlay(canvas, "LEVEL COMPLETE!", Color::RGBA(0,64,0,160))?;
            }
            GamePhase::GameOver => {
                let msg = format!("GAME OVER  Saved: {}/{}", saved, needed);
                Self::draw_overlay(canvas, &msg, Color::RGBA(64,0,0,160))?;
            }
            _ => {}
        }

        canvas.present();
        Ok(())
    }

    fn draw_lemmings(
        canvas:  &mut Canvas<Window>,
        tc:      &TextureCreator<WindowContext>,
        lemmings: &[Lemming],
        frames:  &[Frame],
        cam_x:   i32,
        cam_y:   i32,
        view_h:  i32,
    ) -> Result<()> {
        for lem in lemmings {
            if !lem.active { continue; }

            // Map state+direction to frame index in the sprite sheet
            let row    = lem.sprite_row();
            let f      = lem.frame as usize % 8;
            let mirror = lem.dir == Direction::Left;

            // Each state has 8 frames. Mirrored frames are stored right after.
            // Layout in main file: [state0_right_f0..f7][state0_left_f0..f7][state1_right...] etc.
            let frame_idx = row * 16 + if mirror { 8 + f } else { f };

            if frame_idx >= frames.len() { continue; }
            let frame = &frames[frame_idx];

            let screen_x = lem.x - cam_x - LEMMING_SPRITE_W as i32 / 2;
            let screen_y = lem.y - cam_y - LEMMING_SPRITE_H as i32;

            if screen_x + (LEMMING_SPRITE_W as i32) < 0
            || screen_y + (LEMMING_SPRITE_H as i32) < 0
            || screen_x >= (VIEW_W as i32)
            || screen_y >= view_h {
                continue;
            }

            // Upload this sprite frame as a texture
            let mut spr_tex = tc.create_texture_streaming(
                PixelFormatEnum::RGBA8888,
                frame.w as u32,
                frame.h as u32,
            ).map_err(|e| anyhow::anyhow!("Sprite tex: {}", e))?;
            spr_tex.set_blend_mode(BlendMode::Blend);

            spr_tex.with_lock(None, |pixels, pitch| {
                for row in 0..frame.h {
                    for col in 0..frame.w {
                        let si = (row * frame.w + col) * 4;
                        let di = row * pitch + col * 4;
                        if si + 3 < frame.rgba.len() && di + 3 < pixels.len() {
                            pixels[di]     = frame.rgba[si + 3];
                            pixels[di + 1] = frame.rgba[si + 2];
                            pixels[di + 2] = frame.rgba[si + 1];
                            pixels[di + 3] = frame.rgba[si];
                        }
                    }
                }
            }).map_err(|e| anyhow::anyhow!("Sprite lock: {}", e))?;

            let dst = Rect::new(
                screen_x,
                screen_y,
                frame.w as u32,
                frame.h as u32,
            );
            canvas.copy(&spr_tex, None, dst)
                .map_err(|e| anyhow::anyhow!("Sprite copy: {}", e))?;
        }
        Ok(())
    }

    fn draw_panel(
        canvas:   &mut Canvas<Window>,
        skills:   [u16; 8],
        selected: Option<Skill>,
        saved:    u32,
        _needed:  u16,
        total:    u16,
        time_s:   u32,
        rate:     u32,
    ) -> Result<()> {
        let panel_y  = (WINDOW_H - PANEL_H) as i32;
        let win_rect = Rect::new(0, panel_y, WINDOW_W, PANEL_H);

        // Panel background — dark grey like the Amiga original
        canvas.set_draw_color(Color::RGB(0x30, 0x30, 0x30));
        canvas.fill_rect(win_rect)
            .map_err(|e| anyhow::anyhow!("Panel bg: {}", e))?;

        // ── Skill buttons ────────────────────────────────────────────────────
        let _skill_names = ["CLM","FLT","BOM","BLK","BLD","BSH","MNR","DIG"];
        let skill_colours: [(u8,u8,u8); 8] = [
            (0x00,0xaa,0xff), // Climber  — blue
            (0x00,0xff,0xaa), // Floater  — cyan
            (0xff,0x44,0x44), // Bomber   — red
            (0xff,0xaa,0x00), // Blocker  — orange
            (0xaa,0xff,0x00), // Builder  — green
            (0xff,0x00,0xaa), // Basher   — pink
            (0xaa,0x00,0xff), // Miner    — purple
            (0x00,0xff,0x00), // Digger   — bright green
        ];

        for i in 0..BUTTON_COUNT {
            let bx     = (i as u32 * BUTTON_W) as i32;
            let by     = panel_y;
            let active = selected == Skill::from_index(i);

            let (r, g, b) = skill_colours[i];
            let bg = if active {
                Color::RGB(r.saturating_add(60), g.saturating_add(60), b.saturating_add(60))
            } else if skills[i] == 0 {
                Color::RGB(0x20, 0x20, 0x20)
            } else {
                Color::RGB(r / 3, g / 3, b / 3)
            };

            canvas.set_draw_color(bg);
            canvas.fill_rect(Rect::new(bx, by, BUTTON_W - 2, BUTTON_H - 2))
                .map_err(|e| anyhow::anyhow!("Button fill: {}", e))?;

            // Border
            canvas.set_draw_color(if active { Color::WHITE } else { Color::RGB(0x55,0x55,0x55) });
            canvas.draw_rect(Rect::new(bx, by, BUTTON_W - 2, BUTTON_H - 2))
                .map_err(|e| anyhow::anyhow!("Button border: {}", e))?;

            // Draw count as simple pixel digits (or just squares for brevity here)
            // Full bitmap font rendering is in ui.rs — here we do a minimal colour indicator
            let count_pct = (skills[i] as f32 / 10.0).min(1.0);
            let bar_h     = (BUTTON_H as f32 * count_pct) as u32;
            if bar_h > 0 {
                canvas.set_draw_color(Color::RGB(r, g, b));
                canvas.fill_rect(Rect::new(
                    bx + 2,
                    by + (BUTTON_H - bar_h) as i32 - 2,
                    BUTTON_W - 6,
                    bar_h,
                )).map_err(|e| anyhow::anyhow!("Count bar: {}", e))?;
            }
        }

        // ── Status strip ─────────────────────────────────────────────────────
        let stat_x = (BUTTON_COUNT as u32 * BUTTON_W) as i32;
        let stat_w = WINDOW_W as i32 - stat_x;
        canvas.set_draw_color(Color::RGB(0x18, 0x18, 0x18));
        canvas.fill_rect(Rect::new(stat_x, panel_y, stat_w as u32, PANEL_H))
            .map_err(|e| anyhow::anyhow!("Status bg: {}", e))?;

        // Simple coloured blocks to represent:  SAVED  TIME  RATE
        // Saved bar
        let save_pct = if total > 0 { saved as f32 / total as f32 } else { 0.0 };
        let save_w   = ((stat_w as f32 - 8.0) * save_pct) as u32;
        canvas.set_draw_color(Color::RGB(0x00, 0xcc, 0x44));
        if save_w > 0 {
            canvas.fill_rect(Rect::new(stat_x + 4, panel_y + 4, save_w, 8))
                .map_err(|e| anyhow::anyhow!("Save bar: {}", e))?;
        }

        // Time bar (red when < 60s)
        let time_max   = 600u32;
        let time_pct   = (time_s as f32 / time_max as f32).min(1.0);
        let time_bar_w = ((stat_w as f32 - 8.0) * time_pct) as u32;
        let tcol = if time_s < 60 { Color::RGB(0xff,0x22,0x22) } else { Color::RGB(0xff,0xcc,0x00) };
        canvas.set_draw_color(tcol);
        if time_bar_w > 0 {
            canvas.fill_rect(Rect::new(stat_x + 4, panel_y + 20, time_bar_w, 8))
                .map_err(|e| anyhow::anyhow!("Time bar: {}", e))?;
        }

        // Rate indicator dots
        canvas.set_draw_color(Color::RGB(0x88, 0x88, 0xff));
        for i in 0..(rate / 10).min(10) {
            canvas.fill_rect(Rect::new(
                stat_x + 4 + (i * 12) as i32,
                panel_y + 36,
                8, 8,
            )).map_err(|e| anyhow::anyhow!("Rate dot: {}", e))?;
        }

        Ok(())
    }

    fn draw_overlay(canvas: &mut Canvas<Window>, msg: &str, bg: Color) -> Result<()> {
        let w = WINDOW_W;
        let h = WINDOW_H - PANEL_H;
        canvas.set_draw_color(bg);
        canvas.fill_rect(Rect::new(w as i32 / 4, h as i32 / 2 - 20, w / 2, 40))
            .map_err(|e| anyhow::anyhow!("Overlay: {}", e))?;
        // Message text would render here with bitmap font (see ui.rs)
        let _ = msg;
        Ok(())
    }
}
