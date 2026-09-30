use std::path::Path;
use std::time::{Duration, Instant};

use ruleste::data::atlas::{Atlas, load_atlas_dir};
use ruleste::data::spritebank::SpriteBank;
use ruleste::engine::autotiler::Autotiler;
use ruleste::engine::camera::Camera;
use ruleste::engine::input::Input;
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

/// Parses `--plugin-path=<dir>` and `--show-hitboxes` from the raw argument list,
/// returning the remaining positional args, the plugin dir (defaulting to the
/// `plugins/` folder next to the executable), and the show-hitboxes flag.
/// Keeps the game independent of the cargo `target/` layout.
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

/// Parses the command line into the resolved options plus the remaining
/// positional map/resource paths. Keeps the game independent of the cargo
/// `target/` layout.
struct Cli {
    positional: Vec<String>,
    plugin_dir: String,
    show_hitboxes: bool,
    headless: Option<Headless>,
    log_level: Option<ruleste_core::log::Level>,
}

/// A `--headless` run: simulate without a window and report the outcome on
/// stdout, so it can be driven from CI or a script.
struct Headless {
    /// Fixed simulation timestep in seconds.
    dt: f32,
    /// How many frames to simulate.
    frames: u32,
    /// Stop early once the player entity is gone (a death cycle finished).
    stop_on_despawn: bool,
    /// Optional scripted input: `frames-to-hold:action` steps applied in order.
    script: Vec<ScriptStep>,
}

/// One `--headless-input` entry: hold `action` for `frames` frames, then
/// release it and let the run continue with no action held.
struct ScriptStep {
    frames: u32,
    action: String,
}

const DEFAULT_HEADLESS_DT: f32 = 1.0 / 60.0;
const DEFAULT_HEADLESS_FRAMES: u32 = 600;

impl Headless {
    fn new() -> Headless {
        Headless {
            dt: DEFAULT_HEADLESS_DT,
            frames: DEFAULT_HEADLESS_FRAMES,
            stop_on_despawn: false,
            script: Vec::new(),
        }
    }
}

fn parse_cli_args(raw: Vec<String>) -> Cli {
    let mut plugin_dir = None;
    let mut show_hitboxes = false;
    let mut headless: Option<Headless> = None;
    let mut log_level = None;
    let mut positional: Vec<String> = Vec::with_capacity(raw.len());
    for arg in raw {
        // Any `--headless-*` flag turns headless mode on, and each one only
        // overwrites the field it names, so the flags can appear in any order.
        if arg.starts_with("--headless-") {
            headless = Some(headless.take().unwrap_or_else(Headless::new));
        }
        if let Some(rest) = arg.strip_prefix("--plugin-path=") {
            plugin_dir = Some(rest.to_string());
        } else if let Some(rest) = arg.strip_prefix("--log-level=") {
            log_level = Some(match ruleste_core::log::Level::parse(rest) {
                Some(l) => l,
                None => {
                    eprintln!(
                        "ruleste: unknown --log-level={rest:?}; \
                         expected trace|debug|info|warn|error|off"
                    );
                    std::process::exit(2);
                }
            });
        } else if let Some(rest) = arg.strip_prefix("--headless-frames=") {
            if let Some(h) = headless.as_mut() {
                h.frames = parse_number(rest, "--headless-frames");
            }
        } else if let Some(rest) = arg.strip_prefix("--headless-dt=") {
            if let Some(h) = headless.as_mut() {
                h.dt = parse_number(rest, "--headless-dt");
            }
        } else if let Some(rest) = arg.strip_prefix("--headless-input=") {
            if let Some(h) = headless.as_mut() {
                h.script = parse_input_script(rest);
            }
        } else if arg == "--headless-stop-on-despawn" {
            if let Some(h) = headless.as_mut() {
                h.stop_on_despawn = true;
            }
        } else if arg == "--show-hitboxes" {
            show_hitboxes = true;
        } else if arg == "--help" || arg == "-h" {
            println!(
                r"Usage: ruleste [options] <map> [namespace] [sprites-xml] [tiles-xml] [audio-dir]

Options:
  --plugin-path=<dir>     Plugin directory (default: ./plugins/)
  --show-hitboxes         Show wireframe hitboxes
  --log-level=<level>     trace|debug|info|warn|error|off (default: info,
                          or $RULESTE_LOG_LEVEL)
  --headless-frames=<n>   Simulate n frames with no window and print a report
  --headless-dt=<secs>    Timestep for the headless run (default: 1/60)
  --headless-input=<s>    Scripted input, e.g. 30:move_right,3:jump
                          (each item is <frames-to-hold>:<action>)
  --headless-stop-on-despawn
                          End the headless run as soon as the player is gone
  --help, -h              Show this message

Headless example (no window, no audio device required):
  ruleste --headless-frames=600 --headless-input=40:move_right,3:jump \
          maps/Celeste/0-Intro.bin
"
            );
            std::process::exit(0);
        } else {
            positional.push(arg);
        }
    }
    Cli {
        positional,
        plugin_dir: plugin_dir.unwrap_or_else(default_plugin_dir),
        show_hitboxes,
        headless,
        log_level,
    }
}

/// Parses a numeric CLI value, exiting with a usage error when it is not a
/// number. Keeps the failure path identical for every `--headless-*` flag.
fn parse_number<T: std::str::FromStr>(raw: &str, flag: &str) -> T {
    raw.parse().unwrap_or_else(|_| {
        eprintln!("ruleste: {flag} needs a number, got {raw:?}");
        std::process::exit(2);
    })
}

/// Parses `"30:move_right,3:jump,40:move_left"` into ordered script steps.
/// Each step is held for its frame count, then released, and the run
/// continues with nothing held.
fn parse_input_script(raw: &str) -> Vec<ScriptStep> {
    raw.split(',')
        .filter(|s| !s.is_empty())
        .map(|step| {
            let (frames, action) = step.split_once(':').unwrap_or((step, ""));
            ScriptStep {
                frames: frames.parse().unwrap_or(0),
                action: action.trim().to_ascii_lowercase(),
            }
        })
        .collect()
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
    let cli = parse_cli_args(std::env::args().skip(1).collect());
    // The CLI flag wins over the environment; with neither, the core default
    // (`info`) applies. Applying it before anything loads means the startup
    // banners obey `--log-level` too.
    if let Some(l) = cli.log_level {
        ruleste_core::log::set_level(l);
    } else {
        ruleste_core::log::set_level_from_env();
    }
    ruleste_core::log_info!("ruleste: log level = {}", ruleste_core::log::level());

    let mut args = cli.positional.into_iter();
    let map_path = args
        .next()
        .unwrap_or_else(|| "maps/Celeste/0-Intro.bin".to_string());
    // Maps live at `maps/<pack>/<file>.bin`, converted resources at
    // `resources/<pack>/<namespace>/`. Both default to the pack id (the Celeste
    // converter emits `resources/Celeste/Celeste/...`).
    let pack = Path::new(&map_path)
        .parent()
        .and_then(Path::file_name)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Celeste".to_string());
    let namespace = args.next().unwrap_or_else(|| pack.clone());
    let resources_root = Path::new("resources").join(&pack).join(&namespace);
    let sprite_path = args.next().unwrap_or_default();
    let autotiler_path = args.next().unwrap_or_default();
    let audio_dir = args
        .next()
        .unwrap_or_else(|| resources_root.join("audio").display().to_string());

    // A headless run simulates exactly the same session as the windowed one but
    // never opens a window or an audio device, so it works over SSH and in CI.
    if let Some(h) = cli.headless {
        // Startup chatter is noise for a scripted run; only warnings and errors
        // stay on by default.
        if cli.log_level.is_none() && std::env::var_os("RULESTE_LOG_LEVEL").is_none() {
            ruleste_core::log::set_level(ruleste_core::log::Level::Warn);
        }
        return run_headless(
            h,
            &map_path,
            &namespace,
            &sprite_path,
            &autotiler_path,
            &cli.plugin_dir,
        );
    }

    run_windowed(WindowedArgs {
        map_path,
        namespace,
        sprite_path,
        autotiler_path,
        audio_dir,
        plugin_dir: cli.plugin_dir,
        show_hitboxes: cli.show_hitboxes,
    })
}

/// Everything both the windowed loop and the headless harness need: the
/// decoded assets, the Wasm plugin host with the level's entities spawned, and
/// the camera. Only the *presentation* (window, renderer, audio device) is
/// exclusive to the windowed path, so a headless run can drive `Session`
/// alone with no SDL video or audio device at all.
struct Session {
    /// Shared with the Wasm host, which needs it to resolve an entity's
    /// current atlas frame id for the per-frame metadata FFI.
    atlas: std::sync::Arc<Atlas>,
    sprite_bank: SpriteBank,
    level: ruleste::engine::level::Level,
    autotiler: Autotiler,
    host: WasmHost,
    camera: Camera,
}

impl Session {
    /// Loads the map and its resources, instantiates the plugins the map needs
    /// and spawns the start room (including the single persistent player).
    fn load(
        map_path: &str,
        namespace: &str,
        sprite_path: &str,
        autotiler_path: &str,
        plugin_dir: &str,
    ) -> anyhow::Result<Session> {
        let pack = Path::new(map_path)
            .parent()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Celeste".to_string());
        let resources_root = Path::new("resources").join(&pack).join(namespace);
        let atlas_dir = resources_root.join("textures").join("Atlases");
        let sprite_path = if sprite_path.is_empty() {
            resources_root
                .join("textures")
                .join("Sprites.xml")
                .display()
                .to_string()
        } else {
            sprite_path.to_string()
        };
        let autotiler_path = if autotiler_path.is_empty() {
            resources_root
                .join("textures")
                .join("ForegroundTiles.xml")
                .display()
                .to_string()
        } else {
            autotiler_path.to_string()
        };

        ruleste_core::log_info!("Loading assets...");
        // Merge every atlas in the pack's `Atlases/` directory. Earlier versions
        // hardcoded `Gameplay.meta`; the pack loader also brings in `Misc` and
        // the per-chapter `CompleteScreens` atlases so any frame id referenced by
        // the map, sprites, backdrops or plugins resolves.
        let atlas = std::sync::Arc::new(load_atlas_dir(&atlas_dir).unwrap_or_else(|e| {
            ruleste_core::log_error!("failed to load atlas dir {}: {e}", atlas_dir.display());
            Atlas::default()
        }));
        ruleste_core::log_info!(
            "atlas: {} pages, {} frames",
            atlas.pages.len(),
            atlas.frame_index.len()
        );
        let sprite_bank = SpriteBank::load(Path::new(&sprite_path))?;
        let level = ruleste::engine::level::Level::load(Path::new(map_path))?;
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
            ruleste_core::log_info!(
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
                ruleste_core::log_debug!("  tileset {ts}: {n} tiles");
            }
        }

        let world = ruleste::engine::ecs::World::new();
        let input = Input::default();
        let solids = level.solids.clone();

        ruleste_core::log_info!("Initializing WasmHost from plugin dir: {plugin_dir}");
        let mut host = WasmHost::new(world, input, solids, Path::new(plugin_dir))?;
        host.set_autotiler(autotiler.clone());
        // The FFI needs the bank to resolve an animation's frame count when a
        // plugin calls `Sprite.PlayOffset`.
        host.game_state().sprite_bank = sprite_bank.clone();
        // The FFI resolves an entity's current atlas frame id through this, so
        // plugins can read the per-frame `hair`/`carry` metadata for it.
        host.game_state().atlas = Some(atlas.clone());
        // Instantiate only the plugins whose entity types this level actually
        // contains; unrelated plugins stay unloaded (and uninstantiated). We load
        // the whole map's entity types up front (lazy at map entry), not per room,
        // so a room switch never reloads plugins or re-spawns entities.
        let needed_types: std::collections::HashSet<String> = level
            .rooms
            .iter()
            .flat_map(|r| r.entities.iter().chain(&r.decorations))
            .map(|e| e.name.clone())
            .collect();
        host.load_plugins_for(&needed_types)?;

        // The map is the unit of lazy loading: when a map is entered we
        // instantiate only the plugins its entities need (once). But within a
        // map, only the *current room's* entities are alive — not the whole map —
        // matching Celeste. The `player` is the single Madeline, created exactly
        // once and persisted across room switches; every room's `player` markers
        // are just her respawn points, not separate entities (spawning one per
        // room would create duplicate Madelines on every switch).
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
            .map(|e| {
                (
                    e.name.clone(),
                    offset_spawn(&e.data, start_room.x, start_room.y),
                )
            })
            .collect();

        // Create the single Madeline once, then activate the start room's
        // entities.
        host.spawn_player_once(player_spawn.clone());
        host.enter_room(&start_room_spawns, player_spawn.clone());

        // Warn (once per type) about any entity type across the map with no plugin.
        let all_types: std::collections::HashSet<String> = level
            .rooms
            .iter()
            .flat_map(|r| r.entities.iter().chain(&r.decorations))
            .map(|e| e.name.clone())
            .collect();
        for ty in &all_types {
            if host.plugin_for_type(ty).is_none() {
                ruleste_core::log_warn!("no plugin handles entity type {ty:?}");
            }
        }

        ruleste_core::log_info!(
            "Spawning start room: {} entities and {} decorations (+ 1 player)...",
            start_room.entities.len(),
            start_room.decorations.len()
        );

        let mut camera = Camera::new();
        // Position the camera on the start room. Entities live in world space
        // (spawn data is offset by the room origin), so the camera clamps to the
        // room's world bounds [x, x+width] / [y, y+height].
        {
            let player_world = host
                .game_state()
                .world
                .iter()
                .find(|e| e.entity_type == "player")
                .map(|e| e.position)
                .unwrap_or(Vec2::ZERO);
            camera.snap_to(camera.target_at(
                player_world,
                start_room.camera_offset,
                Vec2::new(start_room.x, start_room.y),
                Vec2::new(start_room.width, start_room.height),
            ));
        }

        Ok(Session {
            atlas,
            sprite_bank,
            level,
            autotiler,
            host,
            camera,
        })
    }
}

/// The resolved, non-headless configuration.
struct WindowedArgs {
    map_path: String,
    namespace: String,
    sprite_path: String,
    autotiler_path: String,
    audio_dir: String,
    plugin_dir: String,
    show_hitboxes: bool,
}

/// Runs the game with a window: loads the session, brings up the SDL renderer
/// and audio device, then drives the render loop.
fn run_windowed(args: WindowedArgs) -> anyhow::Result<()> {
    let dump_frame = std::env::var("RULESTE_DUMP_FRAME").ok();
    let dump_frame_at: u32 = std::env::var("RULESTE_DUMP_FRAME_AT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    if args.show_hitboxes {
        ruleste_core::log_info!("--show-hitboxes — wireframe hitboxes enabled");
    }
    let mut frame_count: u32 = 0;

    ruleste_core::log_info!("Initializing SDL3 renderer...");
    // `RULESTE_DUMP_FRAME` needs the software renderer (readback); without it
    // the game runs on a hardware/GPU driver so maximized scaling stays cheap.
    // Hardware/GPU by default; software only when dumping raw frames.
    let mut renderer = Renderer::new(dump_frame.is_some())?;
    let session = Session::load(
        &args.map_path,
        &args.namespace,
        &args.sprite_path,
        &args.autotiler_path,
        &args.plugin_dir,
    )?;
    let mut audio = setup_audio(&renderer, &args.audio_dir);
    let resources_root = Path::new("resources")
        .join(
            Path::new(&args.map_path)
                .parent()
                .and_then(Path::file_name)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Celeste".to_string()),
        )
        .join(&args.namespace);
    setup_renderer_resources(&mut renderer, &session, &resources_root)?;

    let Session {
        atlas,
        sprite_bank,
        mut level,
        autotiler,
        mut host,
        mut camera,
    } = session;
    let wasm_host = &mut host;

    let tile_grid = autotiler.generate(&level.solids);
    let bg_autotiler_path = Path::new(&args.autotiler_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join("BackgroundTiles.xml");
    let bg_autotiler = Autotiler::load(&bg_autotiler_path)?;
    let bg_tile_grid = bg_autotiler.generate(&level.bg);

    let mut sprite_animator = SpriteAnimator::new(&atlas, &sprite_bank);
    ruleste_core::log_info!("Entering main game loop (ESC to quit)...");

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
            ruleste_core::log_error!("Error reloading plugins: {e}");
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
                    pp.x >= r.x && pp.x < r.x + r.width && pp.y >= r.y && pp.y < r.y + r.height
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
                            (
                                e.name.clone(),
                                offset_spawn(&e.data, new_room.x, new_room.y),
                            )
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
                    camera.snap_to(camera.target_at(
                        pp,
                        new_room.camera_offset,
                        Vec2::new(new_room.x, new_room.y),
                        Vec2::new(new_room.width, new_room.height),
                    ));
                }
            }
        }

        // Feed the audio bus (mix + device). The Wasm plugins queue sounds
        // through `wasm_host.game_state().audio`; drain them into the SDL bus
        // so `event:/...` triggers actually reach the mixer.
        let plugin_sounds = wasm_host.game_state().audio.drain();
        audio.play_plugin_requests(plugin_sounds);
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
        // Apply any pending shake requests from plugins.
        for (intensity, duration) in wasm_host.game_state().shake_requests.drain(..) {
            camera.shake(intensity, duration);
        }
        renderer.set_camera(camera.position);
        // Publish the (shake-adjusted) camera to the host so plugin FFI can
        // convert mouse coordinates into world units.
        let state = wasm_host.game_state();
        state.camera = camera.position;
        state.pixel_scale = renderer.pixel_scale();

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
        renderer.draw_hitboxes(&state.world, args.show_hitboxes);
        renderer.draw_lines(&state.draw_commands);
        renderer.draw_rects(&state.draw_rects);
        renderer.draw_hollow_rects(&state.draw_hollow_rects);
        renderer.draw_circles(&state.draw_circles);
        renderer.draw_texts(&state.draw_texts);
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
                    ruleste_core::log_info!(
                        "RULESTE_DUMP_FRAME: camera at ({:.1}, {:.1})",
                        camera.position.x,
                        camera.position.y
                    );
                } else {
                    ruleste_core::log_error!("RULESTE_DUMP_FRAME: read_pixels failed");
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

/// Uploads the atlas pages, fonts and backdrops the level's resource manifest
/// asks for. Shared by the windowed path; the headless run skips it entirely
/// because it never draws.
fn setup_renderer_resources(
    renderer: &mut Renderer,
    session: &Session,
    resources_root: &Path,
) -> anyhow::Result<()> {
    // Load only the atlas pages the start room's entities actually reference:
    // the core hands the client a manifest of concrete frame ids, and the
    // client uploads just the pages that contain them. Rooms entered later
    // extend the upload on-demand.
    {
        let types: Vec<String> = session
            .level
            .rooms
            .iter()
            .flat_map(|r| r.entities.iter().chain(&r.decorations))
            .map(|e| e.name.clone())
            .collect();
        let mut manifest = ruleste_core::resources::ResourceManifest::for_entity_types(
            &types,
            &session.atlas,
            &session.sprite_bank,
        );
        for b in session
            .level
            .backgrounds
            .iter()
            .chain(&session.level.foregrounds)
        {
            manifest.frames.insert(b.texture.clone());
        }
        let uploaded = renderer.upload_atlas_for(&session.atlas, &manifest.frames)?;
        ruleste_core::log_info!(
            "uploading {uploaded} atlas pages for {} distinct frames (of {} total pages)",
            manifest.frames.len(),
            session.atlas.pages.len()
        );
    }
    // Load the BMFont for plugin-submitted text commands (Draw.Text / OutlineText).
    // Try the dialog font (renogare) first, then fall back to the legacy XNB
    // PressStart2P spritefont. If neither is available, text is a no-op.
    let dialog_font_dir = resources_root.join("texts").join("Fonts");
    let dialog_font_path = dialog_font_dir.join("renogare64.fnt");
    if let Ok(font) = ruleste::data::font::PixelFont::load(&dialog_font_path) {
        let page_count = font
            .sizes
            .iter()
            .flat_map(|s| s.page_textures.iter())
            .count();
        if renderer.upload_pixel_font(font, &dialog_font_dir).is_ok() {
            ruleste_core::log_info!(
                "font: loaded PixelFont (renogare64, {page_count} page references) from {}",
                dialog_font_path.display()
            );
            let uploaded = renderer.font_pages.len();
            if uploaded == 0 {
                ruleste_core::log_warn!(
                    "no font pages uploaded (PNG siblings of .fnt are missing); \
                     Draw.Text will be a no-op until assets are converted"
                );
            }
        } else {
            ruleste_core::log_warn!(
                "font XML parsed but no texture pages found; Draw.Text may be blank"
            );
        }
    }
    // Legacy XNB spritefont — only reached when the BMFont path is absent.
    let spritefont_path = resources_root.join("font").join("PressStart2P.xnb");
    if renderer.font_pages.is_empty() {
        if let Ok(font) = ruleste::data::font::SpriteFont::load(&spritefont_path) {
            ruleste_core::log_info!(
                "font: loaded legacy spritefont from {}",
                spritefont_path.display()
            );
            renderer.set_font(font);
        } else {
            ruleste_core::log_warn!(
                "no dialog font at {} and no spritefont at {}; Draw.Text disabled",
                dialog_font_path.display(),
                spritefont_path.display()
            );
        }
    }
    for b in session
        .level
        .backgrounds
        .iter()
        .chain(&session.level.foregrounds)
    {
        if let Err(e) = renderer.upload_backdrop(&session.atlas, &b.texture) {
            ruleste_core::log_warn!("upload backdrop {}: {e}", b.texture);
        }
    }
    Ok(())
}

/// Opens the audio device and loads the stream manifest.
fn setup_audio(renderer: &Renderer, audio_dir: &str) -> ruleste::engine::audio::AudioBus {
    // Audio: address streams by FMOD/FSB5 name via the audio/ manifest
    // (`event:/...` remapping is a ROADMAP research item). Inert on headless
    // boxes or when the converted tree is absent.
    let mut audio = ruleste::engine::audio::AudioBus::new(&renderer.sdl);
    match ruleste::data::audio::AudioManifest::load(Path::new(audio_dir)) {
        Ok(manifest) => {
            ruleste_core::log_info!(
                "Audio manifest: {} streams across {} banks",
                manifest.len(),
                manifest.banks.len()
            );
            audio.load_manifest(&manifest, Path::new(audio_dir));
        }
        Err(_) => ruleste_core::log_warn!("no audio manifest at {audio_dir:?}; audio disabled"),
    }
    if let Ok(sfx) = std::env::var("RULESTE_PLAY_SFX") {
        if !sfx.trim().is_empty() {
            audio.play(&sfx, 0.6, 0.0, 1.0, false);
        }
    }
    audio
}

/// Simulates the level with no window and no audio device, then prints a
/// machine-readable-ish report on stdout.
///
/// This is the entry point the integration tests use: it drives the real
/// `Session` (assets, plugins, physics) and the real `Input`, so a scripted
/// sequence such as `--headless-input=30:move_right,3:jump` exercises the
/// same code path the game does, minus presentation.
fn run_headless(
    headless: Headless,
    map_path: &str,
    namespace: &str,
    sprite_path: &str,
    autotiler_path: &str,
    plugin_dir: &str,
) -> anyhow::Result<()> {
    let session = Session::load(map_path, namespace, sprite_path, autotiler_path, plugin_dir)?;
    let Session {
        atlas,
        sprite_bank,
        level,
        mut host,
        mut camera,
        ..
    } = session;
    let mut sprite_animator = SpriteAnimator::new(&atlas, &sprite_bank);

    // Turn the script into a per-frame action mask. A step's action is held for
    // its frame count, then released, so a later step can hold a different
    // action independently.
    let mut holds: Vec<Option<&str>> = vec![None; headless.frames as usize];
    let mut frame = 0u32;
    for step in &headless.script {
        for f in frame..frame.saturating_add(step.frames) {
            if let Some(slot) = holds.get_mut(f as usize) {
                *slot = Some(step.action.as_str());
            }
        }
        frame = frame.saturating_add(step.frames);
    }

    let start_pos = player_position(host.game_state());
    let mut end_pos = start_pos;
    let mut despawns = 0u32;
    let mut first_despawn = None;
    let mut simulated = 0u32;
    let mut rooms_visited = 1u32;

    for f in 0..headless.frames {
        apply_scripted_input(&mut host, holds.get(f as usize).copied().flatten());
        // A headless frame runs the same simulation steps as a rendered one,
        // minus drawing: input is already in place, so pump with no events to
        // refresh the virtual-button edges and the latched movement axes.
        host.game_state()
            .input
            .pump(std::iter::empty(), headless.dt);
        host.update(headless.dt);
        // Keep the camera and the sprite animations moving exactly as they would
        // on screen, so any plugin that reads them behaves the same.
        let room = level.room();
        let player = player_position(host.game_state()).unwrap_or(Vec2::ZERO);
        camera.update(
            headless.dt,
            camera.target_at(
                player,
                room.camera_offset,
                Vec2::new(room.x, room.y),
                Vec2::new(room.width, room.height),
            ),
        );
        let state = host.game_state();
        state.camera = camera.position;
        sprite_animator.update(&mut state.world, headless.dt);
        simulated += 1;
        end_pos = player_position(host.game_state());
        rooms_visited = rooms_visited.max(level.current_room as u32 + 1);
        if end_pos.is_none() {
            despawns += 1;
            if first_despawn.is_none() {
                first_despawn = Some(f);
            }
            if headless.stop_on_despawn {
                break;
            }
        }
    }

    // The report goes to stdout so a script can read it, while the leveled logs
    // stay on stderr.
    println!("headless: frames={simulated} dt={:.4}", headless.dt);
    match (start_pos, end_pos) {
        (Some(s), Some(e)) => println!(
            "headless: player start=({:.1}, {:.1}) end=({:.1}, {:.1}) delta=({:+.1}, {:+.1})",
            s.x,
            s.y,
            e.x,
            e.y,
            e.x - s.x,
            e.y - s.y
        ),
        (s, None) => println!(
            "headless: player start={:?} end=despawned (death cycle)",
            s.map(|p| (p.x, p.y))
        ),
        (None, e) => println!(
            "headless: player start=absent end={:?}",
            e.map(|p| (p.x, p.y))
        ),
    }
    println!(
        "headless: despawns={despawns} first_despawn_frame={} room_index={} entities={}",
        first_despawn
            .map(|f| f.to_string())
            .unwrap_or_else(|| "none".to_string()),
        level.current_room,
        host.game_state().world.iter().count()
    );
    let _ = rooms_visited;
    Ok(())
}

/// The player entity's world position, or `None` when she is not in the world.
fn player_position(state: &ruleste::hotload::wasm_host::GameState) -> Option<Vec2> {
    state
        .world
        .iter()
        .find(|e| e.entity_type == "player")
        .map(|e| e.position)
}

/// Applies a scripted action for one frame by injecting the matching key press
/// into the real [`Input`] layer, so the headless run sees the same virtual
/// button edges, buffers and axes a human key press would produce.
fn apply_scripted_input(host: &mut WasmHost, action: Option<&str>) {
    let Some(action) = action else {
        host.game_state().input.scripted_release();
        return;
    };
    host.game_state().input.scripted_press(action);
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
