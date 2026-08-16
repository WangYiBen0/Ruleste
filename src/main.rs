use std::path::Path;
use std::time::{Duration, Instant};

use ruleste::data::atlas::{Atlas, load_atlas_dir};
use ruleste::data::spritebank::SpriteBank;
use ruleste::engine::autotiler::Autotiler;
use ruleste::engine::camera::Camera;
use ruleste::engine::input::Input;
use ruleste::engine::level::Level;
use ruleste::engine::sprites::SpriteAnimator;
use ruleste::hotload::wasm_host::WasmHost;
use ruleste::interface::renderer::Renderer;
use ruleste_plugin_api::types::Vec2;

/// Parses a `--plugin-path=<dir>` style option out of the raw argument list,
/// returning the remaining positional args and the plugin dir (defaulting to
/// the `plugins/` folder next to the executable). Keeps the game independent
/// of the cargo `target/` layout.
fn split_plugin_path(raw: Vec<String>) -> (Vec<String>, String) {
    let mut plugin_dir = None;
    let mut positional: Vec<String> = Vec::with_capacity(raw.len());
    for arg in raw {
        if let Some(rest) = arg.strip_prefix("--plugin-path=") {
            plugin_dir = Some(rest.to_string());
        } else {
            positional.push(arg);
        }
    }
    let dir = plugin_dir.unwrap_or_else(default_plugin_dir);
    (positional, dir)
}

/// The `plugins/` folder next to the running executable.
fn default_plugin_dir() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("plugins")))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "plugins".to_string())
}

fn main() -> anyhow::Result<()> {
    let (positional, plugin_dir) = split_plugin_path(std::env::args().skip(1).collect());
    let mut args = positional.into_iter();
    let map_path = args
        .next()
        .unwrap_or_else(|| "maps/Celeste/0-Intro.bin".to_string());

    // Maps live at `maps/<pack>/<file>.bin`, converted resources at
    // `resources/<pack>/<namespace>/`. Both default to the pack id (the
    // Celeste converter emits `resources/Celeste/Celeste/...`).
    let pack = Path::new(&map_path)
        .parent()
        .and_then(Path::file_name)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Celeste".to_string());
    let namespace = args.next().unwrap_or_else(|| pack.clone());
    let resources_root = Path::new("resources").join(&pack).join(&namespace);
    let atlas_dir = resources_root.join("textures").join("Atlases");
    let sprite_path = args.next().unwrap_or_else(|| {
        resources_root
            .join("textures")
            .join("Sprites.xml")
            .display()
            .to_string()
    });
    let autotiler_path = args.next().unwrap_or_else(|| {
        resources_root
            .join("textures")
            .join("ForegroundTiles.xml")
            .display()
            .to_string()
    });
    let audio_dir = args
        .next()
        .unwrap_or_else(|| resources_root.join("audio").display().to_string());
    let dump_frame = std::env::var("RULESTE_DUMP_FRAME").ok();
    let dump_frame_at: u32 = std::env::var("RULESTE_DUMP_FRAME_AT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut frame_count: u32 = 0;

    println!("Loading assets...");
    // Merge every atlas in the pack's `Atlases/` directory. Earlier versions
    // hardcoded `Gameplay.meta`; the pack loader also brings in `Misc` and the
    // per-chapter `CompleteScreens` atlases so any frame id referenced by the
    // map, sprites, backdrops or plugins resolves.
    let atlas = load_atlas_dir(&atlas_dir).unwrap_or_else(|e| {
        eprintln!(
            "ruleste: failed to load atlas dir {}: {e}",
            atlas_dir.display()
        );
        Atlas::default()
    });
    println!(
        "atlas: {} pages, {} frames",
        atlas.pages.len(),
        atlas.frame_index.len()
    );
    let sprite_bank = SpriteBank::load(Path::new(&sprite_path))?;
    let mut level = Level::load(Path::new(&map_path))?;
    let autotiler = Autotiler::load(Path::new(&autotiler_path))?;
    let bg_autotiler_path = Path::new(&autotiler_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join("BackgroundTiles.xml");
    let bg_autotiler = Autotiler::load(&bg_autotiler_path)?;
    let mut tile_grid = autotiler.generate(&level.room().solids);
    let mut bg_tile_grid = bg_autotiler.generate(&level.room().bg);
    {
        let solid_tiles = level
            .room()
            .solids
            .size()
            .0
            .checked_mul(level.room().solids.size().1)
            .unwrap_or(0);
        let mapped = tile_grid.tileset.iter().filter(|t| !t.is_empty()).count();
        println!(
            "autotiler: {}x{} grid, {} solid tiles, {} tiles mapped to textures",
            level.room().solids.size().0,
            level.room().solids.size().1,
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
    // `RULESTE_DUMP_FRAME` needs the software renderer (readback); without it
    // the game runs on a hardware/GPU driver so maximized scaling stays cheap.
    let mut renderer = Renderer::new(dump_frame.is_some())?;
    renderer.upload_atlas(&atlas)?;
    for b in level.backgrounds.iter().chain(&level.foregrounds) {
        if let Err(e) = renderer.upload_backdrop(&atlas, &b.texture) {
            eprintln!("ruleste: upload backdrop {}: {e}", b.texture);
        }
    }

    // Audio: address streams by FMOD/FSB5 name via the audio/ manifest
    // (`event:/...` remapping is a ROADMAP research item). Inert on headless
    // boxes or when the converted tree is absent.
    let mut audio = ruleste::engine::audio::AudioBus::new(&renderer.sdl);
    if let Ok(manifest) = ruleste::data::audio::AudioManifest::load(Path::new(&audio_dir)) {
        println!(
            "Audio manifest: {} streams across {} banks",
            manifest.len(),
            manifest.banks.len()
        );
        audio.load_manifest(&manifest, Path::new(&audio_dir));
    } else {
        eprintln!("ruleste: no audio manifest at {audio_dir:?}; audio disabled");
    }
    if let Ok(sfx) = std::env::var("RULESTE_PLAY_SFX") {
        if !sfx.trim().is_empty() {
            audio.play(&sfx, 0.6, 0.0, 1.0, false);
        }
    }

    let world = ruleste::engine::ecs::World::new();
    let input = Input::default();
    let solids = level.room().solids.clone();

    println!("Initializing WasmHost from plugin dir: {plugin_dir}");
    let mut wasm_host = WasmHost::new(world, input, solids, Path::new(&plugin_dir))?;
    wasm_host.set_autotiler(autotiler.clone());
    // Instantiate only the plugins whose entity types this level actually
    // contains; unrelated plugins stay unloaded (and uninstantiated).
    let needed_types: std::collections::HashSet<String> = level
        .room()
        .entities
        .iter()
        .chain(&level.room().decorations)
        .map(|e| e.name.clone())
        .collect();
    wasm_host.load_plugins_for(&needed_types)?;

    // Celeste stores every `player` entity as a spawn marker (not a real
    // entity) and creates the single Player at the spawn point closest to the
    // level's bottom-left corner (`Level.DefaultSpawnPoint`). Pick that spawn
    // point here so the initial player position is the intended start, not the
    // first marker in file order (which may be a room-transition respawn).
    let mut all_spawns = Vec::new();
    let mut best_player: Option<&ruleste::engine::level::EntitySpawn> = None;
    for e in &level.room().entities {
        if e.name == "player" {
            let x = e.data.get_float("x", 0.0);
            let y = e.data.get_float("y", 0.0);
            // Distance squared to (0, height): the level's bottom-left corner.
            let dx = x;
            let dy = y - level.room().height;
            let dist = dx * dx + dy * dy;
            let is_better = match &best_player {
                None => true,
                Some(best) => {
                    let bx = best.data.get_float("x", 0.0);
                    let by = best.data.get_float("y", 0.0);
                    let bdx = bx;
                    let bdy = by - level.room().height;
                    let bdist = bdx * bdx + bdy * bdy;
                    dist < bdist
                }
            };
            if is_better {
                best_player = Some(e);
            }
            continue;
        }
        all_spawns.push((e.name.clone(), e.data.to_bytes()));
    }
    if let Some(best) = best_player {
        all_spawns.push((best.name.clone(), best.data.to_bytes()));
    }
    for e in &level.room().decorations {
        all_spawns.push((e.name.clone(), e.data.to_bytes()));
    }

    wasm_host.set_respawn_entities(&all_spawns);

    println!(
        "Spawning {} entities and {} decorations...",
        level.room().entities.len(),
        level.room().decorations.len()
    );
    let mut unhandled: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (name, spawn) in &all_spawns {
        match wasm_host.spawn_entity(name, spawn.clone()) {
            Ok(Some(_)) => {}
            Ok(None) => {
                *unhandled.entry(name.as_str()).or_default() += 1;
            }
            Err(e) => eprintln!("Failed to spawn entity {name:?}: {e}"),
        }
    }
    for (name, count) in &unhandled {
        eprintln!("ruleste: warning: no plugin handles entity type {name:?} ({count} skipped)");
    }

    let mut sprite_animator = SpriteAnimator::new(&atlas, &sprite_bank);
    let mut camera = Camera::new();

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
        wasm_host.game_state().input.pump(events, dt);

        // Hot reload plugins if mtimes changed
        if let Err(e) = wasm_host.reload_plugins() {
            eprintln!("Error reloading plugins: {e}");
        }

        // Update Wasm plugins (physics, player movement, entity logic)
        wasm_host.update(dt);

        // Room transition logic
        let player_pos_opt = wasm_host
            .game_state()
            .world
            .iter()
            .find(|e| e.entity_type == "player")
            .map(|e| e.position);

        if let Some(pp) = player_pos_opt {
            let room = level.room();
            let mut target_room = None;
            let mut new_player_pos = pp;

            if pp.x < room.x {
                if let Some(idx) = level.rooms.iter().position(|r| {
                    (r.x + r.width - room.x).abs() < 2.0 && pp.y >= r.y && pp.y < r.y + r.height
                }) {
                    target_room = Some(idx);
                    new_player_pos.x = level.rooms[idx].x + level.rooms[idx].width - 12.0;
                }
            } else if pp.x >= room.x + room.width {
                if let Some(idx) = level.rooms.iter().position(|r| {
                    (r.x - (room.x + room.width)).abs() < 2.0
                        && pp.y >= r.y
                        && pp.y < r.y + r.height
                }) {
                    target_room = Some(idx);
                    new_player_pos.x = level.rooms[idx].x + 12.0;
                }
            } else if pp.y < room.y {
                if let Some(idx) = level.rooms.iter().position(|r| {
                    (r.y + r.height - room.y).abs() < 2.0 && pp.x >= r.x && pp.x < r.x + r.width
                }) {
                    target_room = Some(idx);
                    new_player_pos.y = level.rooms[idx].y + level.rooms[idx].height - 12.0;
                }
            } else if pp.y >= room.y + room.height {
                if let Some(idx) = level.rooms.iter().position(|r| {
                    (r.y - (room.y + room.height)).abs() < 2.0
                        && pp.x >= r.x
                        && pp.x < r.x + r.width
                }) {
                    target_room = Some(idx);
                    new_player_pos.y = level.rooms[idx].y + 12.0;
                }
            }

            if let Some(idx) = target_room {
                level.current_room = idx;
                let new_room = level.room();
                tile_grid = autotiler.generate(&new_room.solids);
                bg_tile_grid = bg_autotiler.generate(&new_room.bg);

                let mut spawns = Vec::new();
                let mut needed = std::collections::HashSet::new();
                for e in &new_room.entities {
                    needed.insert(e.name.clone());
                    spawns.push((e.name.clone(), e.data.to_bytes()));
                }
                for e in &new_room.decorations {
                    needed.insert(e.name.clone());
                    spawns.push((e.name.clone(), e.data.to_bytes()));
                }
                let _ = wasm_host.load_plugins_for(&needed);
                wasm_host.switch_room(new_room.solids.clone(), &spawns, new_player_pos);
            }
        }

        // Feed the audio bus (mix + device).
        audio.update(dt);

        // Advance parallax backdrops (their `speed` drifts the anchor).
        for b in &mut level.backgrounds {
            b.update(dt);
        }
        for b in &mut level.foregrounds {
            b.update(dt);
        }

        // Follow the player with the level camera.
        let player_pos = wasm_host
            .game_state()
            .world
            .iter()
            .find(|e| e.entity_type == "player")
            .map(|e| e.position)
            .unwrap_or(Vec2::ZERO);
        let room = level.room();
        camera.update(
            dt,
            camera.target_at(
                player_pos,
                room.camera_offset,
                Vec2::new(room.width, room.height),
            ),
        );
        renderer.set_camera(camera.position);

        // Update sprite animations
        let state = wasm_host.game_state();
        sprite_animator.update(&mut state.world, dt);

        // Render frame
        renderer.clear();
        // Draw parallax background layers behind the world
        renderer.draw_backdrops(&level.backgrounds);
        // Draw background layer under entities
        renderer.draw_solids(&bg_tile_grid, &atlas);
        // Draw solid collision layer on top of background
        renderer.draw_solids(&tile_grid, &atlas);
        // Run plugin draw hooks: they set sprite animations and submit custom
        // geometry (e.g. wire cables) before the renderer snapshots the frame.
        wasm_host.draw();
        let state = wasm_host.game_state();
        renderer.draw_entities(&state.world, &atlas, &sprite_bank, &mut sprite_animator);
        renderer.draw_lines(&state.draw_commands);
        renderer.draw_rects(&state.draw_rects);
        renderer.draw_tile_boxes(&state.draw_tile_boxes, &atlas);
        renderer.draw_images(&state.draw_images, &atlas);
        // Parallax foreground layers draw in front of the world
        renderer.draw_backdrops(&level.foregrounds);
        renderer.present();

        // Debug: dump a rendered frame as a PPM and exit.
        if let Some(path) = &dump_frame {
            frame_count += 1;
            if frame_count >= dump_frame_at {
                if let Ok(surface) = renderer.canvas.read_pixels(None) {
                    dump_ppm(&surface, path)?;
                    eprintln!(
                        "RULESTE_DUMP_FRAME: camera at ({:.1}, {:.1})",
                        camera.position.x, camera.position.y
                    );
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
