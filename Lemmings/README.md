# Lemmings — Amiga ADF Native Windows 11 Port

Full Rust port of the original Amiga Lemmings, reading directly from your ADF disk images.
No emulator. Native Windows 11 x64 executable.

---

## Prerequisites

You need your own original ADF disk images:
- `Lemmings.1of2.adf`
- `Lemmings.2of2.adf`

Place both files in the same directory as `lemmings.exe`.

---

## Building from Source

### Requirements

| Tool | Version | Notes |
|------|---------|-------|
| Rust + Cargo | stable ≥ 1.78 | `rustup install stable` |
| MSVC Build Tools | 2022 | For Windows target |
| SDL2 dev headers | 2.30+ | See below |
| NSIS | 3.x | For installer only |

### SDL2 Setup (Windows)

1. Download SDL2-devel-2.30.x-VC.zip from https://github.com/libsdl-org/SDL/releases
2. Download SDL2_mixer-devel-2.8.x-VC.zip from https://github.com/libsdl-org/SDL_mixer/releases
3. Extract both. Copy the `lib\x64\` `.lib` files into a folder, set:

```powershell
$env:SDL2_LIB_DIR = "C:\path\to\sdl2\lib\x64"
$env:SDL2_MIXER_LIB_DIR = "C:\path\to\sdl2_mixer\lib\x64"
```

4. Copy `SDL2.dll` and `SDL2_mixer.dll` from the `lib\x64\` folders into a `sdl2_dlls\` folder at the repo root (used by the NSIS script).

### Compile

```powershell
# Debug build
cargo build

# Release build (optimised, stripped)
cargo build --release --target x86_64-pc-windows-msvc
```

Output: `target\x86_64-pc-windows-msvc\release\lemmings.exe`

### Run directly

```powershell
# Copy ADF files here first, then:
cargo run --release
```

Or place `lemmings.exe`, `SDL2.dll`, `SDL2_mixer.dll`, and both `.adf` files in the same folder and double-click.

---

## Building the Installer

Requires NSIS 3.x installed.

```powershell
# After release build:
cd installer
makensis lemmings.nsi
# Output: installer\LemmingsSetup.exe
```

---

## Controls

| Key / Action | Function |
|---|---|
| Mouse click on lemming | Assign selected skill |
| 1–8 | Select skill (Climber / Floater / Bomber / Blocker / Builder / Basher / Miner / Digger) |
| N | Nuke all lemmings |
| + / - | Increase / decrease release rate |
| ← / → | Scroll level |
| P | Pause |
| F12 | Skip to next level (debug) |
| Escape / Q | Quit |

Mouse near screen edge auto-scrolls the level.

---

## Architecture

```
src/
├── main.rs          Entry point, ADF discovery, SDL2 init
├── adf.rs           Amiga ADF disk reader (custom non-AmigaDOS track layout)
├── decompress.rs    Rob Northen Compression (RNC1) bitstream decompressor
├── level.rs         2048-byte Amiga level format parser (all 25 levels)
├── graphics.rs      4-bitplane planar → RGBA32 converter + palette decoder
├── audio.rs         ProTracker MOD (via SDL2_mixer) + 8-bit PCM SFX engine
├── lemming.rs       Full lemming state machine (14 states, all 8 skills)
├── terrain.rs       Pixel terrain bitmap, destruction, collision + steel masks
├── game.rs          Game loop, spawner, level transitions, input handling
├── renderer.rs      SDL2 terrain streaming texture + sprite blitter
└── ui.rs            Skill panel hit-testing + embedded 5×7 bitmap font
installer/
└── lemmings.nsi     NSIS installer script → LemmingsSetup.exe
```

### ADF Format

Custom non-AmigaDOS disk. Index table at sector byte offset `0x400`:
- Each entry: 14-byte zero-padded name + u32 BE offset + u32 BE compressed size
- 75 files total: `main`, `Code`, `noises`, `Level000`–`Level024`, `Ground1`–`5`, `Objects1`–`5`, `Icons`, `tim1`–`tim10`

### Level Format (2048 bytes, big-endian)

| Offset | Size | Field |
|--------|------|-------|
| 0x000 | u16 | Release rate |
| 0x002 | u16 | Lemming count |
| 0x004 | u16 | Rescue count |
| 0x006 | u16 | Time limit (minutes) |
| 0x008–0x017 | u16×8 | Skill counts (Climb/Float/Bomb/Block/Build/Bash/Mine/Dig) |
| 0x018 | u16 | Start camera X |
| 0x01A | u8 | Style ID (0–4 → Ground1–5) |
| 0x01B | u8 | Music track (1–10 → tim1–tim10) |
| 0x020–0x07F | 4B×32 | Objects (trapdoors / exits / traps) |
| 0x080–0x3FF | 4B×400 | Terrain tile placements |
| 0x400–0x7FF | 4B×32 | Steel area definitions |

### Graphics

Amiga OCS 4-bitplane planar format → 32-bit RGBA:
- 4 bitplanes interleaved by row
- 32-colour palette stored as 12-bit Amiga words (`0x0RGB`) at `0x0560` in Ground files
- Collision mask at `0x0660`

### Audio

- Music: ProTracker MOD files (`tim1`–`tim10`), played via SDL2_mixer's built-in MOD engine
- SFX: 8-bit signed PCM at 11025 Hz, upsampled ×2 to 22050 Hz, served as SDL2 Chunks

---

## License

This port's code is original work. The ADF disk images contain data owned by DMA Design / Psygnosis (1991). Lemmings is widely considered abandonware; no commercial distribution of the ADF files is intended or permitted.
