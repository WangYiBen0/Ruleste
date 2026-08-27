//! Headless behavior checks for the player plugin: jump, dash, and the
//! grab/climb-over-wall state machine. Input is synthesized as SDL key events
//! through `Input::pump`, so the plugin sees the same `held`/`pressed` edges it
//! would in-game.
//!
//! Requires the wasm plugins to be built (see `./build.sh`); skipped otherwise.

use std::path::PathBuf;

use ruleste::engine::ecs::World;
use ruleste::engine::input::Input;
use ruleste::engine::physics::SolidGrid;
use ruleste::hotload::wasm_host::WasmHost;
use ruleste_plugins_api::map::{MapAttr, MapData};
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Mod};

const CLIMB: Keycode = Keycode::Z;
const JUMP: Keycode = Keycode::C;
const DASH: Keycode = Keycode::X;
const DT: f32 = 0.016;
/// Hitbox right edge is `pos.x + oy_offset`; the wall in the tests starts here.
const WALL_X: f32 = 128.0;

fn plugin_dir() -> PathBuf {
    std::env::var_os("RULESTE_PLUGIN_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/wasm32-unknown-unknown/release"))
}

fn has_plugins() -> bool {
    std::fs::read_dir(plugin_dir())
        .map(|it| {
            it.flatten()
                .any(|e| e.path().extension().is_some_and(|e| e == "wasm"))
        })
        .unwrap_or(false)
}

fn spawn_map_attrs(attrs: &[(&str, MapAttr)]) -> Vec<u8> {
    MapData {
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
        nodes: vec![],
    }
    .to_bytes()
}

/// An all-empty tile grid, ensuring out-of-bounds tiles (solid!) stay far away.
fn empty_rows(rows: usize, cols: usize) -> Vec<String> {
    let row = "0".repeat(cols);
    vec![row; rows]
}

/// Floor of solid tiles all the way across at the given row (world y = row*8).
fn floor_rows(rows: usize, cols: usize, floor_row: usize) -> Vec<String> {
    let mut g = empty_rows(rows, cols);
    g[floor_row] = "1".repeat(cols);
    g
}

/// Vertical wall of solid tiles spanning columns [16, 19), full height.
fn wall_rows() -> Vec<String> {
    (0..24)
        .map(|_| {
            (0..20)
                .map(|c| if (16..19).contains(&c) { '1' } else { '0' })
                .collect()
        })
        .collect()
}

fn ref_rows(rows: &[String]) -> Vec<&str> {
    rows.iter().map(|s| s.as_str()).collect()
}

fn ev(kc: Keycode, down: bool) -> Event {
    if down {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(kc),
            scancode: None,
            keymod: Mod::empty(),
            repeat: false,
            which: 0,
            raw: 0,
        }
    } else {
        Event::KeyUp {
            timestamp: 0,
            window_id: 0,
            keycode: Some(kc),
            scancode: None,
            keymod: Mod::empty(),
            repeat: false,
            which: 0,
            raw: 0,
        }
    }
}

fn host_with_solids(rows: &[String]) -> WasmHost {
    let refs = ref_rows(rows);
    WasmHost::new(
        World::new(),
        Input::default(),
        SolidGrid::from_rows(&refs),
        plugin_dir(),
    )
    .unwrap()
}

fn spawn_player(host: &mut WasmHost, x: f32, y: f32) {
    host.spawn_entity(
        "player",
        spawn_map_attrs(&[("x", MapAttr::Float(x)), ("y", MapAttr::Float(y))]),
    )
    .unwrap()
    .unwrap();
}

/// One frame: feed key events (presses and releases in `down`/`up`) then run
/// the host update.
fn frame(host: &mut WasmHost, down: &[Keycode], up: &[Keycode]) {
    let mut events = Vec::new();
    for &kc in down {
        events.push(ev(kc, true));
    }
    for &kc in up {
        events.push(ev(kc, false));
    }
    host.game_state().input.pump(events, DT);
    host.update(DT);
}

fn idle(host: &mut WasmHost) {
    frame(host, &[], &[]);
}

fn pos(host: &mut WasmHost) -> (f32, f32) {
    let e = host
        .game_state()
        .world
        .iter()
        .find(|e| e.entity_type == "player")
        .expect("player spawned");
    (e.position.x, e.position.y)
}

#[test]
fn player_jumps_with_ground() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    let rows = floor_rows(60, 20, 40); // floor top at y=320
    let mut host = host_with_solids(&rows);
    host.load_plugins().unwrap();
    spawn_player(&mut host, 100.0, 320.0);

    idle(&mut host);
    let (_, y0) = pos(&mut host);
    assert!(y0 <= 321.0, "player should rest on the floor: y={y0}");
    frame(&mut host, &[JUMP], &[]);
    for _ in 0..15 {
        idle(&mut host);
    }
    let (_, y1) = pos(&mut host);
    assert!(y1 < y0 - 4.0, "jump should lift the player: y {y0} -> {y1}");
}

#[test]
fn player_dashes_right_from_air() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    let rows = empty_rows(60, 20);
    let mut host = host_with_solids(&rows);
    host.load_plugins().unwrap();
    spawn_player(&mut host, 100.0, 300.0);

    idle(&mut host);
    let (x0, _) = pos(&mut host);
    frame(&mut host, &[DASH], &[]);
    // Don't re-press during the dash; let it end naturally while falling.
    for _ in 0..20 {
        idle(&mut host);
    }
    let (x1, _) = pos(&mut host);
    assert!(x1 > x0 + 15.0, "dash should propel right: x {x0} -> {x1}");
}

#[test]
fn player_grabs_wall_and_holds_height() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    let rows = wall_rows();
    let mut host = host_with_solids(&rows);
    host.load_plugins().unwrap();
    // Left of the wall, falling.
    spawn_player(&mut host, 100.0, 24.0);

    idle(&mut host);
    for _ in 0..40 {
        frame(&mut host, &[CLIMB, Keycode::Right], &[]);
    }
    let (x, y1) = pos(&mut host);
    assert!(
        (x + 4.0 - WALL_X).abs() < 3.0,
        "player should be flush against the wall face: x={x}"
    );
    // Still holding climb+right and not moving up: staying attached stops the
    // fall (running without a grab is a long, fast fall by now).
    assert!(y1 < 120.0, "grab should stop the fall: y={y1}");
    for _ in 0..10 {
        frame(&mut host, &[CLIMB, Keycode::Right], &[]);
    }
    let (_, y2) = pos(&mut host);
    assert!(
        (y2 - y1).abs() < 2.0,
        "climbing still should hold height: {y1} -> {y2}"
    );
}

#[test]
fn player_climbs_up_the_wall() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    let rows = wall_rows();
    let mut host = host_with_solids(&rows);
    host.load_plugins().unwrap();
    spawn_player(&mut host, 100.0, 60.0);

    idle(&mut host);
    // Get attached first (climb+right presses against the wall).
    for _ in 0..25 {
        frame(&mut host, &[CLIMB, Keycode::Right], &[]);
    }
    let (_, y0) = pos(&mut host);
    for _ in 0..25 {
        frame(&mut host, &[CLIMB, Keycode::Right, Keycode::Up], &[]);
    }
    let (_, y1) = pos(&mut host);
    assert!(
        y1 < y0 - 5.0,
        "climbing up should raise the player: y {y0} -> {y1}"
    );
}

#[test]
fn player_clears_short_wall_with_climb_hop() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    // Ground at ys 88.., a low shelf (24px tall) sitting on it.
    // Climbing the shelf with grab+up must hop over its top edge instead of
    // getting stuck against it.
    let mut rows = empty_rows(24, 20);
    for row in rows.iter_mut().skip(11) {
        *row = "1".repeat(20);
    }
    for row in rows.iter_mut().take(11).skip(8) {
        *row = (0..20)
            .map(|c| if (8..11).contains(&c) { '1' } else { '0' })
            .collect();
    }
    let mut host = host_with_solids(&rows);
    host.load_plugins().unwrap();
    spawn_player(&mut host, 40.0, 88.0);

    idle(&mut host);
    let mut cleared = false;
    for _ in 0..200 {
        frame(&mut host, &[CLIMB, Keycode::Right, Keycode::Up], &[]);
        let (_, y) = pos(&mut host);
        // The shelf top is at y=64; the player's 11px body must climb above it.
        if y < 52.0 {
            cleared = true;
            break;
        }
    }
    assert!(
        cleared,
        "grab+up should hop a 24px low wall over its top edge"
    );
}
