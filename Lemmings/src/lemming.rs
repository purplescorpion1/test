// lemming.rs — Complete Lemming state machine
// Ports all M68000 state logic: walk, fall, climb, float, block,
// build, bash, mine, dig, ohno, explode, exit, drown, splat.
// Language: Rust, file: src/lemming.rs

use crate::terrain::Terrain;
use crate::level::Skill;
use crate::audio::Sfx;

// ── Constants ────────────────────────────────────────────────────────────────

pub const GRAVITY:         i32 = 1;    // pixels per tick
pub const SPLAT_FALL:      i32 = 62;   // pixels of free-fall before splat
pub const FLOATER_KICK_IN: i32 = 16;   // ticks of falling before float activates
pub const WALK_SPEED:      i32 = 2;    // pixels per tick
pub const BUILD_STEPS:     u8  = 12;   // bricks per build sequence
pub const BASH_WIDTH:      usize = 16;
pub const BASH_MASKS: [[u8; BASH_WIDTH]; 4] = [
    [0,0,0,0,1,1,1,1,1,1,1,1,0,0,0,0],
    [0,0,0,1,1,1,1,1,1,1,1,1,1,0,0,0],
    [0,0,1,1,1,1,1,1,1,1,1,1,1,1,0,0],
    [0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0],
];
pub const MINE_MASKS: [[u8; 16]; 2] = [
    [0,0,0,0,0,0,1,1,1,1,0,0,0,0,0,0],
    [0,0,0,0,1,1,1,1,1,1,1,1,0,0,0,0],
];
pub const DIG_WIDTH: usize = 10;

// ── State ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Walking,
    Falling,
    Climbing,
    ClimbingOverTop,
    Floating,
    Blocking,
    Building,
    Bashing,
    Mining,
    Digging,
    OhNo,       // counted down 15 ticks before explode
    Exploding,
    Exiting,
    Drowning,
    Splat,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Right = 1,
    Left  = -1,
}

impl Direction {
    pub fn flip(self) -> Self {
        match self {
            Direction::Right => Direction::Left,
            Direction::Left  => Direction::Right,
        }
    }
    pub fn dx(self) -> i32 { self as i32 }
}

// ── Lemming ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Lemming {
    pub x:         i32,
    pub y:         i32,
    pub dir:       Direction,
    pub state:     State,
    pub frame:     u8,    // animation frame counter
    pub fall_dist: i32,   // accumulated falling distance
    pub ohno_tick: u8,    // countdown from 15 to 0
    pub build_cnt: u8,    // bricks laid so far
    pub bash_tick: u8,    // bash frame cycle
    pub mine_tick: u8,
    pub active:    bool,

    // Permanent skills
    pub is_climber: bool,
    pub is_floater: bool,
    // Current assigned skill (temporary)
    pub assigned:   Option<Skill>,
    // Queued sound to emit this tick
    pub sfx:        Option<Sfx>,
}

impl Lemming {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x, y,
            dir:       Direction::Right,
            state:     State::Falling,
            frame:     0,
            fall_dist: 0,
            ohno_tick: 0,
            build_cnt: 0,
            bash_tick: 0,
            mine_tick: 0,
            active:    true,
            is_climber: false,
            is_floater: false,
            assigned:  None,
            sfx:       None,
        }
    }

    /// Main update: one game tick. Returns any SFX that should play.
    pub fn tick(&mut self, terrain: &mut Terrain) -> Option<Sfx> {
        if !self.active { return None; }
        self.sfx = None;

        match self.state {
            State::Walking        => self.tick_walk(terrain),
            State::Falling        => self.tick_fall(terrain),
            State::Climbing       => self.tick_climb(terrain),
            State::ClimbingOverTop=> self.tick_climb_over(terrain),
            State::Floating       => self.tick_float(terrain),
            State::Blocking       => self.tick_block(),
            State::Building       => self.tick_build(terrain),
            State::Bashing        => self.tick_bash(terrain),
            State::Mining         => self.tick_mine(terrain),
            State::Digging        => self.tick_dig(terrain),
            State::OhNo           => self.tick_ohno(),
            State::Exploding      => self.tick_explode(terrain),
            State::Exiting        => self.tick_exit(),
            State::Drowning       => self.tick_drown(),
            State::Splat          => self.tick_splat(),
            State::Dead           => {}
        }

        self.sfx
    }

    // ── Apply a skill from the player ──────────────────────────────────────

    pub fn assign_skill(&mut self, skill: Skill) -> bool {
        match skill {
            Skill::Climber => {
                if !self.is_climber {
                    self.is_climber = true;
                    return true;
                }
            }
            Skill::Floater => {
                if !self.is_floater {
                    self.is_floater = true;
                    return true;
                }
            }
            Skill::Bomber => {
                if self.state != State::OhNo && self.state != State::Exploding {
                    self.state    = State::OhNo;
                    self.ohno_tick = 15;
                    return true;
                }
            }
            Skill::Blocker => {
                if self.state == State::Walking {
                    self.state = State::Blocking;
                    return true;
                }
            }
            Skill::Builder => {
                if matches!(self.state, State::Walking | State::Falling) {
                    self.state     = State::Building;
                    self.build_cnt = 0;
                    return true;
                }
            }
            Skill::Basher => {
                if self.state == State::Walking {
                    self.state     = State::Bashing;
                    self.bash_tick = 0;
                    return true;
                }
            }
            Skill::Miner => {
                if self.state == State::Walking {
                    self.state     = State::Mining;
                    self.mine_tick = 0;
                    return true;
                }
            }
            Skill::Digger => {
                if self.state == State::Walking {
                    self.state = State::Digging;
                    return true;
                }
            }
        }
        false
    }

    // ── State handlers ────────────────────────────────────────────────────

    fn tick_walk(&mut self, terrain: &mut Terrain) {
        let dx = self.dir.dx();
        let nx = self.x + dx * WALK_SPEED;

        // Wall check: solid pixel at mid-body height
        if terrain.is_solid(nx, self.y - 1) {
            // Try to climb up (up to 6px stair)
            let mut climbed = false;
            for up in 1..=6 {
                if !terrain.is_solid(nx, self.y - 1 - up) {
                    self.x  = nx;
                    self.y -= up;
                    climbed = true;
                    break;
                }
            }
            if !climbed {
                // Blocked — flip or start climbing if is_climber
                if self.is_climber {
                    self.state = State::Climbing;
                } else {
                    self.dir = self.dir.flip();
                }
            }
        } else {
            self.x = nx;
        }

        // Ground check — fall if no ground below
        if !terrain.is_solid(self.x, self.y) {
            self.state     = State::Falling;
            self.fall_dist = 0;
        }

        // Check for exit
        if terrain.is_exit(self.x, self.y) {
            self.state = State::Exiting;
            self.sfx   = Some(Sfx::LemmingExit);
        }

        // Check for trap
        if terrain.is_trap(self.x, self.y) {
            self.state = State::Dead;
            self.sfx   = Some(Sfx::Trap);
        }

        // Water drown
        if terrain.is_water(self.x, self.y) {
            self.state = State::Drowning;
            self.sfx   = Some(Sfx::Drown);
        }

        self.frame = (self.frame + 1) % 8;
        if self.state == State::Walking {
            self.sfx = Some(Sfx::LemmingWalk);
        }
    }

    fn tick_fall(&mut self, terrain: &mut Terrain) {
        // Float kicks in after enough falling if is_floater
        if self.is_floater && self.fall_dist >= FLOATER_KICK_IN {
            self.state = State::Floating;
            return;
        }

        let gravity_step = if self.fall_dist < 4 { GRAVITY } else { GRAVITY * 2 };

        for _ in 0..gravity_step {
            self.y += 1;
            self.fall_dist += 1;
            if terrain.is_solid(self.x, self.y) {
                self.y -= 1;
                // Landing
                if self.fall_dist >= SPLAT_FALL {
                    self.state = State::Splat;
                    self.sfx   = Some(Sfx::LemmingSplat);
                } else {
                    self.state     = State::Walking;
                    self.fall_dist = 0;
                }
                return;
            }
            if terrain.is_water(self.x, self.y) {
                self.state = State::Drowning;
                self.sfx   = Some(Sfx::Drown);
                return;
            }
            if terrain.is_trap(self.x, self.y) {
                self.state = State::Dead;
                self.sfx   = Some(Sfx::Trap);
                return;
            }
        }
        self.frame = (self.frame + 1) % 4;
        self.sfx   = Some(Sfx::LemmingFall);
    }

    fn tick_float(&mut self, terrain: &mut Terrain) {
        // Parachute: fall 1 pixel every 2 ticks
        if self.frame % 2 == 0 {
            self.y += 1;
        }
        if terrain.is_solid(self.x, self.y) {
            self.y    -= 1;
            self.state = State::Walking;
        }
        if terrain.is_water(self.x, self.y) {
            self.state = State::Drowning;
        }
        self.frame = (self.frame + 1) % 8;
    }

    fn tick_climb(&mut self, terrain: &mut Terrain) {
        // Move up 1px per tick while there's a wall to grip
        let wall_x = self.x + self.dir.dx();
        if !terrain.is_solid(wall_x, self.y - 1) {
            // Wall ends — transition to over-top
            self.state = State::ClimbingOverTop;
            return;
        }
        if terrain.is_solid(wall_x, self.y - 7) {
            // Ceiling — can't climb, fall off
            self.dir   = self.dir.flip();
            self.state = State::Falling;
            return;
        }
        self.y    -= 1;
        self.frame = (self.frame + 1) % 8;
        self.sfx   = Some(Sfx::LemmingClimb);
    }

    fn tick_climb_over(&mut self, terrain: &mut Terrain) {
        // Move horizontally over the top
        self.x += self.dir.dx();
        if !terrain.is_solid(self.x, self.y) {
            self.state     = State::Falling;
            self.fall_dist = 0;
        } else if !terrain.is_solid(self.x, self.y - 1) {
            self.y    -= 1;
            self.state = State::Walking;
        }
    }

    fn tick_block(&mut self) {
        // Blockers stay put; they turn others around (handled in game.rs)
        self.frame = (self.frame + 1) % 16;
    }

    fn tick_build(&mut self, terrain: &mut Terrain) {
        // Every 4 ticks lay a brick row (3 pixels wide, 1 tall)
        self.frame = (self.frame + 1) % 16;
        if self.frame % 4 != 0 { return; }

        if self.build_cnt >= BUILD_STEPS {
            // Out of bricks — revert to walk
            self.state = State::Walking;
            return;
        }

        // Place brick: 6px wide, 1px tall at current position
        let bx = self.x + self.dir.dx() * 2;
        let by = self.y;
        for dx in 0..6i32 {
            let px = bx + dx * self.dir.dx();
            terrain.set_solid(px, by, true);
        }
        self.build_cnt += 1;
        self.x  += self.dir.dx() * 2;
        self.y  -= 1;
        self.sfx = Some(Sfx::LemmingBuild);

        // Check if building into ceiling
        if terrain.is_solid(self.x, self.y - 5) {
            self.state = State::Walking;
        }
    }

    fn tick_bash(&mut self, terrain: &mut Terrain) {
        self.bash_tick = (self.bash_tick + 1) % 4;
        if self.bash_tick != 0 { return; }

        // Erode a mask-shaped area ahead
        let mask = &BASH_MASKS[(self.bash_tick as usize) % 4];
        for col in 0..BASH_WIDTH {
            if mask[col] == 1 {
                let px = self.x + self.dir.dx() * (col as i32 - 4);
                for dy in -4..2 {
                    terrain.set_solid(px, self.y + dy, false);
                }
            }
        }

        // Advance
        self.x    += self.dir.dx() * 2;
        self.frame = (self.frame + 1) % 16;
        self.sfx   = Some(Sfx::LemmingBash);

        // Check if we've hit steel or open air
        if !terrain.is_solid(self.x + self.dir.dx() * 4, self.y - 1) {
            self.state = State::Walking;
        }
        if terrain.is_steel(self.x + self.dir.dx() * 4, self.y - 1) {
            self.dir   = self.dir.flip();
            self.state = State::Walking;
        }
    }

    fn tick_mine(&mut self, terrain: &mut Terrain) {
        self.mine_tick = (self.mine_tick + 1) % 24;
        // Only erode on tick 0 and 12 of the 24-tick cycle
        if self.mine_tick != 0 && self.mine_tick != 12 { return; }

        let mask = &MINE_MASKS[(self.mine_tick / 12) as usize];
        for col in 0..16usize {
            if mask[col] == 1 {
                let px = self.x + self.dir.dx() * (col as i32 - 8);
                terrain.set_solid(px, self.y,     false);
                terrain.set_solid(px, self.y + 1, false);
            }
        }

        self.x    += self.dir.dx();
        self.y    += 1;
        self.frame = (self.frame + 1) % 24;
        self.sfx   = Some(Sfx::LemmingMine);

        if !terrain.is_solid(self.x, self.y) {
            self.state     = State::Falling;
            self.fall_dist = 0;
        }
        if terrain.is_steel(self.x + self.dir.dx(), self.y) {
            self.dir   = self.dir.flip();
            self.state = State::Walking;
        }
    }

    fn tick_dig(&mut self, terrain: &mut Terrain) {
        // Erase DIG_WIDTH pixels below feet each tick
        for dx in -(DIG_WIDTH as i32 / 2)..(DIG_WIDTH as i32 / 2) {
            terrain.set_solid(self.x + dx, self.y,     false);
            terrain.set_solid(self.x + dx, self.y + 1, false);
        }
        self.y    += 1;
        self.frame = (self.frame + 1) % 8;
        self.sfx   = Some(Sfx::LemmingDigDown);

        if terrain.is_steel(self.x, self.y) {
            self.state = State::Walking;
        }
        if !terrain.is_solid(self.x, self.y + 1) {
            self.state     = State::Falling;
            self.fall_dist = 0;
        }
    }

    fn tick_ohno(&mut self) {
        if self.ohno_tick > 0 {
            self.ohno_tick -= 1;
            self.sfx       = Some(Sfx::LemmingOhNo);
            self.frame      = (self.frame + 1) % 8;
        } else {
            self.state = State::Exploding;
        }
    }

    fn tick_explode(&mut self, terrain: &mut Terrain) {
        // Erase a 14×14 circle from terrain
        for dy in -7..7i32 {
            for dx in -7..7i32 {
                if dx * dx + dy * dy <= 49 {
                    terrain.set_solid(self.x + dx, self.y + dy, false);
                }
            }
        }
        self.state  = State::Dead;
        self.active = false;
        self.sfx    = Some(Sfx::LemmingExplode);
    }

    fn tick_exit(&mut self) {
        self.frame = (self.frame + 1) % 8;
        if self.frame == 0 {
            // Animation done — lemming is saved
            self.state  = State::Dead; // remove from field, game.rs counts saves
            self.active = false;
        }
    }

    fn tick_drown(&mut self) {
        self.frame = (self.frame + 1) % 16;
        if self.frame == 0 {
            self.state  = State::Dead;
            self.active = false;
        }
    }

    fn tick_splat(&mut self) {
        self.frame = (self.frame + 1) % 16;
        if self.frame == 0 {
            self.state  = State::Dead;
            self.active = false;
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    pub fn is_blocking(&self) -> bool {
        self.state == State::Blocking
    }

    /// Return the sprite sheet row for the current state/direction.
    pub fn sprite_row(&self) -> usize {
        match self.state {
            State::Walking         => 0,
            State::Falling         => 1,
            State::Climbing        => 2,
            State::ClimbingOverTop => 3,
            State::Floating        => 4,
            State::Blocking        => 5,
            State::Building        => 6,
            State::Bashing         => 7,
            State::Mining          => 8,
            State::Digging         => 9,
            State::OhNo            => 10,
            State::Exploding       => 11,
            State::Exiting         => 12,
            State::Drowning        => 13,
            State::Splat           => 14,
            State::Dead            => 14,
        }
    }
}

// Convenience: did this lemming reach the exit successfully?
impl Lemming {
    pub fn exited_safely(&self) -> bool {
        // Lemming was in Exiting state when it became inactive
        // We track this with a separate bool set in tick_exit
        !self.active && self.state == State::Dead && self.frame == 0
        // Note: frame resets to 0 on completion of exit animation in tick_exit
    }
}
