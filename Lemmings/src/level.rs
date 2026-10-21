// level.rs — Amiga Lemmings 2048-byte level format parser
// Language: Rust, file: src/level.rs

use anyhow::{bail, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

pub const LEVEL_SIZE:      usize = 0x800; // 2048 bytes
pub const MAX_OBJECTS:     usize = 32;    // traps + exits + trapdoors
pub const MAX_TERRAIN:     usize = 400;   // terrain tile placements
pub const MAX_STEEL:       usize = 32;    // steel areas

// 8 skills in order as stored in level header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skill {
    Climber  = 0,
    Floater  = 1,
    Bomber   = 2,
    Blocker  = 3,
    Builder  = 4,
    Basher   = 5,
    Miner    = 6,
    Digger   = 7,
}

impl Skill {
    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Skill::Climber),
            1 => Some(Skill::Floater),
            2 => Some(Skill::Bomber),
            3 => Some(Skill::Blocker),
            4 => Some(Skill::Builder),
            5 => Some(Skill::Basher),
            6 => Some(Skill::Miner),
            7 => Some(Skill::Digger),
            _ => None,
        }
    }
}

// Object types found in the object array (0x020–0x07F)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Trapdoor = 0,  // lemming spawner
    Exit     = 1,  // salvation door
    Trap     = 2,  // killer trap
}

#[derive(Debug, Clone)]
pub struct LevelObject {
    pub x:    i16,
    pub y:    i16,
    pub kind: ObjectKind,
    pub id:   u8,   // which trap/exit sprite from the graphics set
    pub flags: u8,
}

#[derive(Debug, Clone)]
pub struct TerrainTile {
    pub x:        i16,
    pub y:        i16,
    pub tile_id:  u8,
    pub modifier: u8,  // flip | erase | noOverwrite flags
}

impl TerrainTile {
    pub fn flip_x(&self)       -> bool { self.modifier & 1 != 0 }
    pub fn erase(&self)        -> bool { self.modifier & 2 != 0 }
    pub fn no_overwrite(&self) -> bool { self.modifier & 4 != 0 }
}

#[derive(Debug, Clone)]
pub struct SteelArea {
    pub x:    i16,
    pub y:    i16,
    pub w:    u8,
    pub h:    u8,
}

#[derive(Debug, Clone)]
pub struct Level {
    // Header 0x000–0x01F
    pub release_rate:   u16,          // 0x000
    pub lemming_count:  u16,          // 0x002
    pub rescue_count:   u16,          // 0x004  how many must be saved
    pub time_limit:     u16,          // 0x006  minutes
    pub skills:         [u16; 8],     // 0x008–0x017
    pub start_x:        u16,          // 0x018  initial camera x
    pub style_id:       u8,           // 0x01A  Ground1–5 graphics set index (0-based)
    pub music_track:    u8,           // 0x01B  which tim (1-based)

    // Objects 0x020–0x07F
    pub objects: Vec<LevelObject>,

    // Terrain 0x080–0x3FF  (each entry 4 bytes, 0xFFFF terminates)
    pub terrain: Vec<TerrainTile>,

    // Steel 0x400–0x7FF  (each entry 4 bytes, 0xFFFF terminates)
    pub steel: Vec<SteelArea>,
}

impl Level {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < LEVEL_SIZE {
            bail!(
                "Level data too short: {} bytes (need {})",
                data.len(), LEVEL_SIZE
            );
        }
        let mut cur = Cursor::new(data);

        // ── Header ───────────────────────────────────────────────────────────
        let release_rate  = cur.read_u16::<BigEndian>()?;
        let lemming_count = cur.read_u16::<BigEndian>()?;
        let rescue_count  = cur.read_u16::<BigEndian>()?;
        let time_limit    = cur.read_u16::<BigEndian>()?;

        let mut skills = [0u16; 8];
        for s in &mut skills {
            *s = cur.read_u16::<BigEndian>()?;
        }

        let start_x     = cur.read_u16::<BigEndian>()?;
        let _start_y    = cur.read_u16::<BigEndian>()?;  // always 0
        let style_id    = cur.read_u8()?;
        let music_track = cur.read_u8()?;
        // Padding to 0x020
        let _ = cur.read_u32::<BigEndian>()?;
        let _ = cur.read_u32::<BigEndian>()?;

        // ── Objects (0x020–0x07F, 16 × 4-byte entries) ──────────────────────
        let mut objects = Vec::new();
        for _ in 0..MAX_OBJECTS {
            let word0 = cur.read_u16::<BigEndian>()?;
            let word1 = cur.read_u16::<BigEndian>()?;

            if word0 == 0xFFFF { break; }

            // word0: [kind:2][id:6][x_high:1][x:7]   (approximate)
            // Actual Lemmings encoding (verified against level000):
            //   bits 15-14: object kind
            //   bits 13- 8: object id within style
            //   bits  7- 0: x coordinate high byte
            // word1 = x low | y
            let kind_bits = (word0 >> 14) & 0x3;
            let id        = ((word0 >> 8) & 0x3F) as u8;
            let x_hi      = (word0 & 0xFF) as i16;
            let x_lo      = ((word1 >> 8) & 0xFF) as i16;
            let y         = (word1 & 0xFF) as i16;
            let x         = (x_hi << 8) | x_lo;
            let flags     = 0u8;

            let kind = match kind_bits {
                0 => ObjectKind::Trapdoor,
                1 => ObjectKind::Exit,
                _ => ObjectKind::Trap,
            };

            objects.push(LevelObject { x, y, kind, id, flags });
        }

        // ── Terrain tiles (0x080–0x3FF, 4 bytes each) ───────────────────────
        cur.set_position(0x080);
        let mut terrain = Vec::new();
        for _ in 0..MAX_TERRAIN {
            let word0 = cur.read_u16::<BigEndian>()?;
            let word1 = cur.read_u16::<BigEndian>()?;

            if word0 == 0xFFFF { break; }

            // word0: modifier[3] | x[10] | unused[3]
            // word1: tile_id[8]  | y[8]
            let modifier = ((word0 >> 13) & 0x7) as u8;
            let x        = ((word0 >> 3) & 0x3FF) as i16 - 16;
            let tile_id  = ((word1 >> 8) & 0xFF) as u8;
            let y        = (word1 & 0xFF) as i16;

            terrain.push(TerrainTile { x, y, tile_id, modifier });
        }

        // ── Steel areas (0x400–0x7FF, 4 bytes each) ─────────────────────────
        cur.set_position(0x400);
        let mut steel = Vec::new();
        for _ in 0..MAX_STEEL {
            let word0 = cur.read_u16::<BigEndian>()?;
            let word1 = cur.read_u16::<BigEndian>()?;

            if word0 == 0 && word1 == 0 { break; }

            // word0: x[9] | y[7]   word1: w[4] | h[4] | unused
            let x = ((word0 >> 7) & 0x1FF) as i16 * 2;
            let y = (word0 & 0x7F) as i16 * 2;
            let w = (((word1 >> 12) & 0xF) as u8 + 1) * 4;
            let h = (((word1 >>  8) & 0xF) as u8 + 1) * 4;

            steel.push(SteelArea { x, y, w, h });
        }

        Ok(Level {
            release_rate,
            lemming_count,
            rescue_count,
            time_limit,
            skills,
            start_x,
            style_id,
            music_track,
            objects,
            terrain,
            steel,
        })
    }

    pub fn skill_count(&self, s: Skill) -> u16 {
        self.skills[s as usize]
    }

    pub fn trapdoor_pos(&self) -> Option<(i16, i16)> {
        self.objects.iter()
            .find(|o| o.kind == ObjectKind::Trapdoor)
            .map(|o| (o.x, o.y))
    }
}
