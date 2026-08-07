//! Spikes entity plugin — the level's primary hazard.
//!
//! One plugin handles all four orientations (`spikesUp`, `spikesDown`,
//! `spikesLeft`, `spikesRight`). Each spike row is `width`/`height` pixels of
//! 8x8 tiles drawn from the `danger/spikes/<type>_<direction>` atlas frames;
//! the collision rect is the 3px-deep sliver in front of the row, mirroring
//! `Spikes.cs`. Contact kills the player, respecting the directional check in
//! the original `OnCollide` (e.g. upward spikes only kill when falling).

use ruleste_plugin_api::host::{self, die, draw_image, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("spikes");
ruleste_plugin_api::ruleste_entity_types!("spikesUp", "spikesDown", "spikesLeft", "spikesRight");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Spike depth in pixels (the collision sliver in front of the row).
const THICKNESS: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn suffix(self) -> &'static str {
        match self {
            Dir::Up => "up",
            Dir::Down => "down",
            Dir::Left => "left",
            Dir::Right => "right",
        }
    }
}

#[derive(Debug)]
struct SpikeState {
    dir: Dir,
    spike_type: String,
    size: f32,
}
thread_local! {
    static STATES: std::cell::RefCell<EntityState<SpikeState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SpikeState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, || unreachable!("state seeded at init")) as *mut SpikeState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn dir_for_type(entity_type: &str) -> Dir {
    match entity_type {
        "spikesDown" => Dir::Down,
        "spikesLeft" => Dir::Left,
        "spikesRight" => Dir::Right,
        _ => Dir::Up,
    }
}

#[no_mangle]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity_type = spawn.get_str("_entity_type", "spikesUp");
    let dir = dir_for_type(&entity_type);
    let spike_type = spawn.get_str("type", "default");
    let size = match dir {
        Dir::Up | Dir::Down => spawn.get_float("width", 8.0),
        Dir::Left | Dir::Right => spawn.get_float("height", 8.0),
    };
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(-1);

    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            SpikeState {
                dir,
                spike_type,
                size,
            },
        );
    });
}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let _ = dt;
    let (dir, size) = with_state(id, |st| (st.dir, st.size));
    let spike = Entity::new(id);
    let sp = spike.position.get();

    let (sx, sy, sw, sh) = match dir {
        Dir::Up => (sp.x, sp.y - THICKNESS, size, THICKNESS),
        Dir::Down => (sp.x, sp.y, size, THICKNESS),
        Dir::Left => (sp.x - THICKNESS, sp.y, THICKNESS, size),
        Dir::Right => (sp.x, sp.y, THICKNESS, size),
    };

    for player_id in entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let px = pp.x + pox;
        let py = pp.y + poy;

        let overlap = px < sx + sw && px + pw > sx && py < sy + sh && py + ph > sy;
        if !overlap {
            continue;
        }
        let vel = host::Speed::new(player_id).get();
        let kills = match dir {
            Dir::Up => vel.y >= 0.0,
            Dir::Down => vel.y <= 0.0,
            Dir::Left => vel.x >= 0.0,
            Dir::Right => vel.x <= 0.0,
        };
        if kills {
            die();
        }
        break;
    }
}

#[no_mangle]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let sp = entity.position.get();
        let tiles = (st.size / 8.0).max(1.0) as usize;
        for j in 0..tiles {
            let frame = format!("danger/spikes/{}_{}00", st.spike_type, st.dir.suffix());
            let (cx, cy) = match st.dir {
                Dir::Up => (sp.x + (j as f32 + 0.5) * 8.0, sp.y - 3.5),
                Dir::Down => (sp.x + (j as f32 + 0.5) * 8.0, sp.y + 3.5),
                Dir::Left => (sp.x - 3.5, sp.y + (j as f32 + 0.5) * 8.0),
                Dir::Right => (sp.x + 3.5, sp.y + (j as f32 + 0.5) * 8.0),
            };
            draw_image(&frame, cx, cy, 0.0, 1.0, 1.0);
        }
    });
}
