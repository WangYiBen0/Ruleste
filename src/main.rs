use std::path::Path;
use std::time::{Duration, Instant};

use ruleste::data::atlas::Atlas;
use ruleste::data::spritebank::SpriteBank;
use ruleste::engine::autotiler::Autotiler;
use ruleste::engine::input::Input;
use ruleste::engine::level::Level;
use ruleste::engine::sprites::SpriteAnimator;
use ruleste::hotload::wasm_host::WasmHost;
use ruleste::interface::renderer::Renderer;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let map_path = args
        .next()
        .unwrap_or_else(|| "references/Celeste/Content/Maps/0-Intro.bin".to_string());
    let atlas_path = args
        .next()
        .unwrap_or_else(|| "references/Celeste/Content/Graphics/Atlases/Gameplay.meta".to_string());
    let sprite_path = args
        .next()
        .unwrap_or_else(|| "references/Celeste/Content/Graphics/Sprites.xml".to_string());
    let autotiler_path = args
        .next()
        .unwrap_or_else(|| "references/Celeste/Content/Graphics/ForegroundTiles.xml".to_string());
    let plugin_dir = args
        .next()
        .unwrap_or_else(|| "target/wasm32-unknown-unknown/release".to_string());
    let dump_frame = std::env::var("RULESTE_DUMP_FRAME").ok();
    let dump_frame_at: u32 = std::env::var("RULESTE_DUMP_FRAME_AT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut frame_count: u32 = 0;

    println!("Loading assets...");
    let atlas = Atlas::load(Path::new(&atlas_path))?;
    let sprite_bank = SpriteBank::load(Path::new(&sprite_path))?;
    let level = Level::load(Path::new(&map_path))?;
    let autotiler = Autotiler::load(Path::new(&autotiler_path))?;
    let tile_grid = autotiler.generate(&level.solids);
    {
        let solid_tiles = level
            .solids
            .size()
            .0
            .checked_mul(level.solids.size().1)
            .unwrap_or(0);
        let mapped = tile_grid.tileset.iter().filter(|t| !t.is_empty()).count();
        println!(
            "autotiler: {}x{} grid, {} solid tiles, {} tiles mapped to textures",
            level.solids.size().0,
            level.solids.size().1,
            solid_tiles,
            mapped
        );
        let mut tilesets: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for t in &tile_grid.tileset {
            if !t.is_empty() {
                *tilesets.entry(t).or_default() += 1;
            }
        }
        for (ts, n) in &tilesets {
            println!("  tileset {ts}: {n} tiles");
        }
    }

    println!("Initializing SDL3 renderer...");
    let mut renderer = Renderer::new()?;
    renderer.upload_atlas(&atlas)?;

    let world = ruleste::engine::ecs::World::new();
    let input = Input::default();
    let solids = level.solids.clone();

    println!("Initializing WasmHost from plugin dir: {plugin_dir}");
    let mut wasm_host = WasmHost::new(world, input, solids, Path::new(&plugin_dir))?;
    wasm_host.load_plugins()?;

    println!("Spawning {} level entities...", level.entities.len());
    let mut unhandled: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for entity in &level.entities {
        match wasm_host.spawn_entity(&entity.name, entity.data.to_bytes()) {
            Ok(Some(_)) => {}
            Ok(None) => {
                *unhandled.entry(&entity.name).or_default() += 1;
            }
            Err(e) => eprintln!("Failed to spawn entity {:?}: {e}", entity.name),
        }
    }
    for (name, count) in &unhandled {
        eprintln!("ruleste: warning: no plugin handles entity type {name:?} ({count} skipped)");
    }

    let mut sprite_animator = SpriteAnimator::new(&atlas, &sprite_bank);

    println!("Entering main game loop (ESC to quit)...");
    let target_fps = 60.0;
    let frame_duration = Duration::from_secs_f32(1.0 / target_fps);
    let mut last_time = Instant::now();

    'running: loop {
        let frame_start = Instant::now();
        let dt = last_time.elapsed().as_secs_f32().min(0.1);
        last_time = frame_start;

        // Poll SDL events once; handle quit before feeding the rest to input.
        let events: Vec<sdl3::event::Event> = renderer.pump.poll_iter().collect();
        for event in &events {
            if let sdl3::event::Event::Quit { .. } = event {
                break 'running;
            }
            if let sdl3::event::Event::KeyDown {
                keycode: Some(sdl3::keyboard::Keycode::Escape),
                ..
            } = event
            {
                break 'running;
            }
        }
        wasm_host.game_state().input.pump(events);

        // Hot reload plugins if mtimes changed
        if let Err(e) = wasm_host.reload_plugins() {
            eprintln!("Error reloading plugins: {e}");
        }

        // Update Wasm plugins (physics, player movement, entity logic)
        wasm_host.update(dt);

        // Update sprite animations
        let state = wasm_host.game_state();
        sprite_animator.update(&mut state.world, dt);

        // Render frame
        renderer.canvas.clear();
        renderer.draw_solids(&tile_grid, &atlas);
        let state = wasm_host.game_state();
        renderer.draw_entities(&state.world, &atlas, &sprite_bank, &mut sprite_animator);
        renderer.present();

        // Debug: dump a rendered frame as a PPM and exit.
        if let Some(path) = &dump_frame {
            frame_count += 1;
            if frame_count >= dump_frame_at {
                if let Ok(surface) = renderer.canvas.read_pixels(None) {
                    dump_ppm(&surface, path)?;
                } else {
                    eprintln!("RULESTE_DUMP_FRAME: read_pixels failed");
                }
                break 'running;
            }
        }

        // Frame rate limiter
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    Ok(())
}

/// Writes an SDL surface as an RGB PPM (P6). Used for headless frame
/// debugging via `RULESTE_DUMP_FRAME`.
fn dump_ppm(surface: &sdl3::surface::Surface, path: &str) -> anyhow::Result<()> {
    let w = surface.width();
    let h = surface.height();
    let pitch = surface.pitch() as usize;

    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    surface.with_lock(|bytes| {
        // 32-bit surfaces have pitch >= w*4; 24-bit surfaces pitch == w*3.
        let bytes_per_pixel = if pitch >= w as usize * 4 { 4 } else { 3 };
        for y in 0..h as usize {
            let row = &bytes[y * pitch..y * pitch + w as usize * bytes_per_pixel];
            for x in 0..w as usize {
                let px = &row[x * bytes_per_pixel..x * bytes_per_pixel + bytes_per_pixel];
                match bytes_per_pixel {
                    4 => {
                        // SDL_RenderReadPixels returns ARGB8888 surfaces, whose
                        // little-endian byte order is B,G,R,A.
                        let (b, g, r) = (px[0], px[1], px[2]);
                        rgb.extend_from_slice(&[r, g, b]);
                    }
                    _ => rgb.extend_from_slice(&px[..3]),
                }
            }
        }
    });

    let mut out = std::fs::File::create(path)?;
    use std::io::Write;
    writeln!(out, "P6\n{w} {h}\n255")?;
    out.write_all(&rgb)?;
    println!("dumped frame to {path} ({w}x{h})");
    Ok(())
}
