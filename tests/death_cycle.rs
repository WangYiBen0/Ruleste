//! End-to-end death cycle: the spikes plugin kills the player, the host
//! freezes the room, and respawns it from the recorded spawn recipes.
//!
//! Requires the wasm plugins to be built (`cargo build -p ... --target
//! wasm32-unknown-unknown --release`); the test skips when they are absent.

use std::path::Path;

use ruleste::engine::ecs::World;
use ruleste::engine::input::Input;
use ruleste::engine::physics::SolidGrid;
use ruleste::hotload::wasm_host::WasmHost;
use ruleste_plugin_api::map::{MapAttr, MapData};

const PLUGIN_DIR: &str = "target/wasm32-unknown-unknown/release";

fn spawn_map(x: f32, y: f32) -> Vec<u8> {
    let data = MapData {
        attrs: vec![
            ("x".to_string(), MapAttr::Float(x)),
            ("y".to_string(), MapAttr::Float(y)),
        ],
        nodes: vec![],
    };
    data.to_bytes()
}

fn has_plugins() -> bool {
    std::fs::read_dir(PLUGIN_DIR)
        .map(|it| {
            it.flatten()
                .any(|e| e.path().extension().is_some_and(|e| e == "wasm"))
        })
        .unwrap_or(false)
}

#[test]
fn spike_kills_and_room_respawns() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    let mut host = WasmHost::new(
        World::new(),
        Input::default(),
        SolidGrid::from_rows(&[]),
        Path::new(PLUGIN_DIR),
    )
    .unwrap();
    host.load_plugins().unwrap();

    let recipes = vec![
        ("player".to_string(), spawn_map(60.0, 168.0)),
        (
            "spikesUp".to_string(),
            MapData {
                attrs: vec![
                    ("x".to_string(), MapAttr::Float(40.0)),
                    ("y".to_string(), MapAttr::Float(168.0)),
                    ("width".to_string(), MapAttr::Byte(40)),
                ],
                nodes: vec![],
            }
            .to_bytes(),
        ),
    ];
    host.set_respawn_entities(&recipes);
    for (ty, spawn) in &recipes {
        host.spawn_entity(ty, spawn.clone()).unwrap().unwrap();
    }

    // Player spawns at (60,168); after a few frames the falling player overlaps
    // the spikeUp row at (40,168) and the spikes plugin kills it.
    for _ in 0..5 {
        host.update(0.01);
    }
    assert!(
        host.game_state().death_timer > 0.0,
        "player should be dead after touching spikes"
    );
    let player = host
        .game_state()
        .world
        .iter()
        .find(|e| e.entity_type == "player")
        .expect("player exists");
    assert!(!player.visible, "player hidden during death freeze");

    // Let the death freeze elapse; the room is rebuilt and the player is back
    // at its spawn position.
    let mut elapsed = 0.0;
    while elapsed < 2.0 {
        host.update(0.1);
        elapsed += 0.1;
        if host.game_state().death_timer <= 0.0 && elapsed > 1.0 {
            break;
        }
    }
    assert!(
        host.game_state().death_timer <= 0.0,
        "death freeze should have ended"
    );
    let player = host
        .game_state()
        .world
        .iter()
        .find(|e| e.entity_type == "player")
        .expect("player respawned");
    assert!(player.visible, "player visible after respawn");
    assert!(
        (player.position.x - 60.0).abs() < 0.01 && (player.position.y - 168.0).abs() < 0.01,
        "player respawned at spawn position, got {:?}",
        player.position
    );
}

#[test]
fn collected_strawberry_does_not_respawn() {
    if !has_plugins() {
        eprintln!("skipping: wasm plugins not built");
        return;
    }
    let mut host = WasmHost::new(
        World::new(),
        Input::default(),
        SolidGrid::from_rows(&[]),
        Path::new(PLUGIN_DIR),
    )
    .unwrap();
    host.load_plugins().unwrap();

    let recipes = vec![
        ("player".to_string(), spawn_map(60.0, 157.0)),
        ("strawberry".to_string(), spawn_map(300.0, 100.0)),
    ];
    host.set_respawn_entities(&recipes);
    for (ty, spawn) in &recipes {
        host.spawn_entity(ty, spawn.clone()).unwrap().unwrap();
    }

    // Collect: simulate the strawberry plugin consuming its own entity.
    let berry = host
        .game_state()
        .world
        .iter()
        .find(|e| e.entity_type == "strawberry")
        .map(|e| e.id)
        .expect("strawberry spawned");
    let berry_spawn = {
        let state = host.game_state();
        state.world.get(berry).unwrap().spawn.clone()
    };
    host.game_state().collected.insert(berry_spawn);
    host.despawn(berry);

    // Kill the player to force a respawn.
    host.kill_player();
    let mut elapsed = 0.0;
    while elapsed < 2.0 {
        host.update(0.1);
        elapsed += 0.1;
        if host.game_state().death_timer <= 0.0 && elapsed > 1.0 {
            break;
        }
    }
    let berries = host
        .game_state()
        .world
        .iter()
        .filter(|e| e.entity_type == "strawberry")
        .count();
    assert_eq!(berries, 0, "collected strawberry stays gone after respawn");
    assert_eq!(
        host.game_state().world.iter().count(),
        1,
        "only the player respawns"
    );
}
