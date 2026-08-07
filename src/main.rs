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

    println!("Loading assets...");
    let atlas = Atlas::load(Path::new(&atlas_path))?;
    let sprite_bank = SpriteBank::load(Path::new(&sprite_path))?;
    let level = Level::load(Path::new(&map_path))?;
    let autotiler = Autotiler::load(Path::new(&autotiler_path))?;
    let tile_grid = autotiler.generate(&level.solids);

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

        // Poll SDL events & pump input
        {
            let state = wasm_host.game_state();
            state.input.pump(&mut renderer.pump);
        }

        // Check quit events
        for event in renderer.pump.poll_iter() {
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

        // Frame rate limiter
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    Ok(())
}
