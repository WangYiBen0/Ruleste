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
use ruleste_plugins_api::map::{MapAttr, MapData};
use ruleste_plugins_api::types::Vec2;

// Celeste stores each entity's spawn `x`/`y` (and nodes) relative to its room's
// top-left corner. The engine keeps every room in its own *local* coordinate
// space: the active room's solid/background grids, the camera bounds and all
// entity positions are local to that room. Room switches simply swap which
// room is active, so no world-origin shifting of entity spawns is needed.
// `pp` (the player position used for room transitions) is converted to world
// space only for the rectangle-containment test against room world rects.

/// Parses a `--plugin-path=<dir>` style option out of the raw argument list,
/// returning the remaining positional args and the plugin dir (defaulting to
/// the `plugins/` folder next to the executable). Keeps the game independent
/// of the cargo `target/` layout.
/// Celeste rooms are laid out in world space, each anchored at its own origin
/// `(x, y)`. Entity spawn data stores *local* coordinates (relative to the
/// room's top-left), so before handing a spawn to a plugin we must translate
/// both the `x`/`y` position and every `node` by the room origin. The plugin
/// then operates entirely in world space, which matches the composite collision
/// grid (`Level.solids`) and the camera. Nodes round-trip through
/// `MapData::to_bytes`/`from_bytes`, so offsetting them here is lossless.
fn offset_spawn(d: &MapData, ox: f32, oy: f32) -> Vec<u8> {
    let mut d = d.clone();
    let x = d.get_float("x", 0.0) + ox;
    let y = d.get_float("y", 0.0) + oy;
    for (k, v) in d.attrs.iter_mut() {
        if k == "x" {
            *v = MapAttr::Float(x);
        } else if k == "y" {
            *v = MapAttr::Float(y);
        }
    }
    for n in d.nodes.iter_mut() {
        n.x += ox;
        n.y += oy;
    }
    d.to_bytes()
}

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

/// Squared distance from a world point to a room rectangle (0 when inside).
/// Used to pick the nearest room when the player exits the current one into a
/// gap between room rectangles.
fn room_rect_dist2(r: &ruleste::engine::level::Room, p: Vec2) -> f32 {
    let dx = if p.x < r.x {
        r.x - p.x
    } else if p.x > r.x + r.width {
        p.x - (r.x + r.width)
    } else {
        0.0
    };
    let dy = if p.y < r.y {
        r.y - p.y
    } else if p.y > r.y + r.height {
        p.y - (r.y + r.height)
    } else {
        0.0
    };
    dx * dx + dy * dy
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
    let tile_grid = autotiler.generate(&level.solids);
    let bg_tile_grid = bg_autotiler.generate(&level.bg);
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
    let solids = level.solids.clone();

    println!("Initializing WasmHost from plugin dir: {plugin_dir}");
    let mut wasm_host = WasmHost::new(world, input, solids, Path::new(&plugin_dir))?;
    wasm_host.set_autotiler(autotiler.clone());
    // Instantiate only the plugins whose entity types this level actually
    // contains; unrelated plugins stay unloaded (and uninstantiated). We load the
    // whole map's entity types up front (lazy at map entry), not per room, so a
    // room switch never reloads plugins or re-spawns entities.
    let needed_types: std::collections::HashSet<String> = level
        .rooms
        .iter()
        .flat_map(|r| r.entities.iter().chain(&r.decorations))
        .map(|e| e.name.clone())
        .collect();
    wasm_host.load_plugins_for(&needed_types)?;

    // The map is the unit of lazy loading: when a map is entered we instantiate
    // only the plugins its entities need (once). But within a map, only the
    // *current room's* entities are alive — not the whole map — matching
    // Celeste. The `player` is the single Madeline, created exactly once and
    // persisted across room switches; every room's `player` markers are just her
    // respawn points, not separate entities (spawning one per room would create
    // duplicate Madelines on every switch).
    let start_room = level.room();
    let start_x = start_room.x;
    let start_y = start_room.y + start_room.height;
    let mut best_player: Option<(f32, f32, Vec<u8>)> = None;
    for e in start_room.entities.iter().chain(&start_room.decorations) {
        if e.name == "player" {
            let wx = e.data.get_float("x", 0.0) + start_room.x;
            let wy = e.data.get_float("y", 0.0) + start_room.y;
            let dx = wx - start_x;
            let dy = wy - start_y;
            let dist = dx * dx + dy * dy;
            let better = match &best_player {
                None => true,
                Some((bwx, bwy, _)) => {
                    let bdx = *bwx - start_x;
                    let bdy = *bwy - start_y;
                    dist < bdx * bdx + bdy * bdy
                }
            };
            if better {
                best_player = Some((wx, wy, offset_spawn(&e.data, start_room.x, start_room.y)));
            }
        }
    }
    let player_spawn = match best_player {
        Some((_, _, bytes)) => ("player".to_string(), bytes),
        None => ("player".to_string(), Vec::new()),
    };
    let start_room_spawns: Vec<(String, Vec<u8>)> = start_room
        .entities
        .iter()
        .chain(&start_room.decorations)
        .filter(|e| e.name != "player")
        .map(|e| (e.name.clone(), offset_spawn(&e.data, start_room.x, start_room.y)))
        .collect();

    // Create the single Madeline once, then activate the start room's entities.
    wasm_host.spawn_player_once(player_spawn.clone());
    wasm_host.enter_room(&start_room_spawns, player_spawn.clone());

    // Warn (once per type) about any entity type across the map with no plugin.
    let all_types: std::collections::HashSet<String> = level
        .rooms
        .iter()
        .flat_map(|r| r.entities.iter().chain(&r.decorations))
        .map(|e| e.name.clone())
        .collect();
    for ty in &all_types {
        if wasm_host.plugin_for_type(ty).is_none() {
            eprintln!("ruleste: warning: no plugin handles entity type {ty:?}");
        }
    }

    println!(
        "Spawning start room: {} entities and {} decorations (+ 1 player)...",
        start_room.entities.len(),
        start_room.decorations.len()
    );

    let mut sprite_animator = SpriteAnimator::new(&atlas, &sprite_bank);
    let mut camera = Camera::new();
    // Position the camera on the start room. Entities live in world space (spawn
    // data is offset by the room origin), so the camera clamps to the room's world
    // bounds [x, x+width] / [y, y+height].
    {
        let start_room = level.room();
        let player_world = wasm_host
            .game_state()
            .world
            .iter()
            .find(|e| e.entity_type == "player")
            .map(|e| e.position)
            .unwrap_or(Vec2::ZERO);
        camera.position = camera.target_at(
            player_world,
            start_room.camera_offset,
            Vec2::new(start_room.x, start_room.y),
            Vec2::new(start_room.width, start_room.height),
        );
    }

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

        // Room transition: Celeste rooms overlap and are not edge-tiled, so the
        // active room is simply whichever room rectangle contains the player.
        // When the player leaves the current room and enters another, we switch
        // to that room (reloading its entities) and snap the camera; the player
        // keeps its current world position rather than being warped to a spawn
        // marker.
        let player_pos_opt = wasm_host
            .game_state()
            .world
            .iter()
            .find(|e| e.entity_type == "player")
            .map(|e| e.position);

        if let Some(pp) = player_pos_opt {
            // `pp` is the player position in world space (entities are offset by
            // the room origin at spawn, matching the composite collision grid).
            let cur = level.current_room;
            let cur_room = &level.rooms[cur];
            let in_cur = pp.x >= cur_room.x
                && pp.x < cur_room.x + cur_room.width
                && pp.y >= cur_room.y
                && pp.y < cur_room.y + cur_room.height;
            if !in_cur {
                // Prefer the room whose rectangle actually contains the player's
                // world point (an exact doorway). Celeste rooms tile contiguously
                // in the source data, but a few converted levels leave small gaps
                // between room rects, so fall back to the *nearest* room when the
                // exit point lands in a gap — otherwise the player would walk out
                // of a room and never switch.
                let exact = level.rooms.iter().position(|r| {
                    pp.x >= r.x
                        && pp.x < r.x + r.width
                        && pp.y >= r.y
                        && pp.y < r.y + r.height
                });
                // Fallback only for *near* rooms (a real doorway leaves at most a
                // pixel-sized gap between contiguous room rects). This keeps pit
                // falls — where the player is far below every room — switching to
                // a distant room instead of respawning.
                const FALLBACK_MAX_DIST2: f32 = 16.0 * 16.0;
                let target = exact.or_else(|| {
                    level
                        .rooms
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != cur)
                        .map(|(i, r)| (i, room_rect_dist2(r, pp)))
                        .filter(|(_, d)| *d < FALLBACK_MAX_DIST2)
                        .min_by(|(_, a), (_, b)| a.total_cmp(b))
                        .map(|(i, _)| i)
                });
                if let Some(idx) = target {
                    level.current_room = idx;
                    let new_room = level.room();
                    // Only the new room's entities become active (the map is the lazy
                    // unit, but within it Celeste keeps just the current room alive —
                    // not the whole map). `enter_room` despawns the previous room's
                    // entities and spawns the new room's, leaving the single Madeline
                    // untouched, so no duplicate player is created. Her respawn point
                    // is updated to this room's player marker.
                    let new_room_spawns: Vec<(String, Vec<u8>)> = new_room
                        .entities
                        .iter()
                        .chain(&new_room.decorations)
                        .filter(|e| e.name != "player")
                        .map(|e| {
                            (e.name.clone(), offset_spawn(&e.data, new_room.x, new_room.y))
                        })
                        .collect();
                    let new_player = new_room
                        .entities
                        .iter()
                        .chain(&new_room.decorations)
                        .filter(|e| e.name == "player")
                        .map(|e| offset_spawn(&e.data, new_room.x, new_room.y))
                        .next()
                        .unwrap_or_default();
                    wasm_host.enter_room(&new_room_spawns, ("player".to_string(), new_player));
                    // The player keeps its continuous world position as it walks
                    // through the doorway; we only snap the camera to the new room.
                    camera.position = camera.target_at(
                        pp,
                        new_room.camera_offset,
                        Vec2::new(new_room.x, new_room.y),
                        Vec2::new(new_room.width, new_room.height),
                    );
                }
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
                Vec2::new(room.x, room.y),
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
