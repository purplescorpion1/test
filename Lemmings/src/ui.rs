// ui.rs — Skill panel UI, hit-testing, embedded bitmap font
// Language: Rust, file: src/ui.rs

use crate::level::Skill;
use crate::renderer::{BUTTON_W, BUTTON_H, BUTTON_COUNT, WINDOW_H, PANEL_H};

// ── Embedded 5×7 bitmap font for digits 0–9 ─────────────────────────────────
const FONT_W: usize = 5;
const FONT_H: usize = 7;

const DIGIT_FONT: [[u8; FONT_H]; 10] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111], // 2
    [0b11111, 0b00010, 0b00100, 0b00110, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b01110, 0b10001, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b00100, 0b00100, 0b00100], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b10001, 0b01110], // 9
];

pub fn render_digit(buf: &mut Vec<u8>, buf_w: usize, digit: u8, x: usize, y: usize, r: u8, g: u8, b: u8) {
    if digit > 9 { return; }
    let rows = &DIGIT_FONT[digit as usize];
    for (row_i, bits) in rows.iter().enumerate() {
        for col in 0..FONT_W {
            if (bits >> (FONT_W - 1 - col)) & 1 == 1 {
                let px  = x + col;
                let py  = y + row_i;
                let idx = (py * buf_w + px) * 4;
                if idx + 3 < buf.len() {
                    buf[idx]     = r;
                    buf[idx + 1] = g;
                    buf[idx + 2] = b;
                    buf[idx + 3] = 255;
                }
            }
        }
    }
}

pub fn render_number(buf: &mut Vec<u8>, buf_w: usize, n: u16, x: usize, y: usize, r: u8, g: u8, b: u8) {
    render_digit(buf, buf_w, (n / 10 % 10) as u8, x,     y, r, g, b);
    render_digit(buf, buf_w, (n % 10)       as u8, x + 6, y, r, g, b);
}

// ── Skill button ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct SkillButton {
    pub skill:  Skill,
    pub rect_x: i32,
    pub rect_y: i32,
}

impl SkillButton {
    fn hit(&self, x: i32, y: i32) -> bool {
        x >= self.rect_x && x < self.rect_x + BUTTON_W as i32
        && y >= self.rect_y && y < self.rect_y + BUTTON_H as i32
    }
}

// ── Ui ───────────────────────────────────────────────────────────────────────

pub struct Ui {
    pub buttons: Vec<SkillButton>,
}

impl Ui {
    pub fn new() -> Self {
        let panel_y = (WINDOW_H - PANEL_H) as i32;
        let buttons = (0..BUTTON_COUNT)
            .filter_map(|i| {
                Skill::from_index(i).map(|skill| SkillButton {
                    skill,
                    rect_x: (i as u32 * BUTTON_W) as i32,
                    rect_y: panel_y,
                })
            })
            .collect();
        Self { buttons }
    }

    pub fn hit_test(&self, x: i32, y: i32, _win_h: i32) -> Option<Skill> {
        self.buttons.iter().find(|b| b.hit(x, y)).map(|b| b.skill)
    }
}
