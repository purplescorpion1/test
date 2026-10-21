// main.rs — entry point
// Lemmings: Amiga ADF → Windows 11 native Rust port
// Language: Rust, file: src/main.rs, runtime: Windows 11 x64

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adf;
mod decompress;
mod level;
mod graphics;
mod audio;
mod lemming;
mod terrain;
mod game;
mod renderer;
mod ui;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::env;

fn find_adf_files() -> Result<(PathBuf, PathBuf)> {
    let search_dirs = vec![
        PathBuf::from("."),
        env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
    ];

    for dir in &search_dirs {
        let disk1 = dir.join("Lemmings.1of2.adf");
        let disk2 = dir.join("Lemmings.2of2.adf");
        if disk1.exists() && disk2.exists() {
            return Ok((disk1, disk2));
        }
        // Case-insensitive fallback names
        for entry in std::fs::read_dir(dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
            if let Ok(e) = entry {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name == "lemmings.1of2.adf" {
                    let d1 = e.path();
                    let d2 = dir.join(
                        std::fs::read_dir(dir)
                            .unwrap()
                            .filter_map(|x| x.ok())
                            .find(|x| x.file_name().to_string_lossy().to_lowercase() == "lemmings.2of2.adf")
                            .map(|x| x.path())
                            .unwrap_or_default(),
                    );
                    return Ok((d1, d2));
                }
            }
        }
    }
    anyhow::bail!(
        "Could not find Lemmings.1of2.adf and Lemmings.2of2.adf.\n\
         Place both ADF files in the same directory as the executable."
    )
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    log::info!("Lemmings starting — Spirit Port");

    let (adf1_path, adf2_path) = find_adf_files()
        .context("ADF disk images not found")?;

    log::info!("Disk 1: {:?}", adf1_path);
    log::info!("Disk 2: {:?}", adf2_path);

    let adf1 = std::fs::read(&adf1_path)
        .with_context(|| format!("Cannot read {:?}", adf1_path))?;
    let adf2 = std::fs::read(&adf2_path)
        .with_context(|| format!("Cannot read {:?}", adf2_path))?;

    let disk = adf::Disk::load(&adf1, &adf2)
        .context("Failed to parse ADF disk images")?;

    log::info!("Loaded {} files from ADF", disk.file_count());

    let sdl = sdl2::init().map_err(|e| anyhow::anyhow!("SDL2 init: {}", e))?;
    let video = sdl.video().map_err(|e| anyhow::anyhow!("SDL2 video: {}", e))?;
    let _audio_sys = sdl.audio().map_err(|e| anyhow::anyhow!("SDL2 audio: {}", e))?;

    sdl2::mixer::open_audio(22050, sdl2::mixer::AUDIO_S8, 1, 512)
        .map_err(|e| anyhow::anyhow!("SDL2 mixer: {}", e))?;
    sdl2::mixer::allocate_channels(8);

    let window = video
        .window("Lemmings", renderer::WINDOW_W, renderer::WINDOW_H)
        .position_centered()
        .resizable()
        .build()
        .context("Window creation failed")?;

    let canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .context("Canvas creation failed")?;

    let texture_creator = canvas.texture_creator();

    let event_pump = sdl.event_pump()
        .map_err(|e| anyhow::anyhow!("Event pump: {}", e))?;

    let mut game = game::Game::new(disk, canvas, texture_creator, event_pump)
        .context("Game init failed")?;

    game.run()
}
