#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `spinner` hazard plugin.
//!
//! Mirrors `DustStaticSpinner.cs`, which is what the `spinner` map entity
//! becomes in area 3 (Celestial Resort) and the `d-` chapters of area 7: a
//! slowly rotating dust crystal. Contact kills Madeline.
//!
//! The `DustGraphic` is simplified but keeps the original structure: a
//! `danger/dustcreature/center` crystal at the entity position (spinning),
//! plus four `base`+`overlay` nodes on the diagonal corners. Each corner is
//! skipped when solid tiles occupy it, and nodes stretch outward (x5) when
//! another spinner or wall sits 16px away, mirroring `autoExpandDust`.

use ruleste_plugin_api::host::{self, die, draw_image, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("spinner");
ruleste_plugin_api::ruleste_entity_types!("spinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Approximate `ColliderList(new Circle(6f), new Hitbox(16f, 4f, -8f, -3f))`
/// as one AABB covering both shapes (centered on the position).
const KILL_W: f32 = 16.0;
const KILL_H: f32 = 12.0;
/// Corner offsets for the four diagonal dust nodes.
const CORNERS: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

const CENTER_FRAMES: [&str; 1] = ["danger/dustcreature/center00"];
const BASE_FRAMES: [&str; 3] = [
    "danger/dustcreature/base00",
    "danger/dustcreature/base01",
    "danger/dustcreature/base02",
];
const OVERLAY_FRAMES: [&str; 3] = [
    "danger/dustcreature/overlay00",
    "danger/dustcreature/overlay01",
    "danger/dustcreature/overlay02",
];

/// `DustGraphic` update speeds.
const CENTER_SPIN: f32 = 0.6; // radians/sec
const NODE_SPIN: f32 = 0.5; // radians/sec

#[derive(Debug)]
struct SpinnerState {
    center_timer: f32,
    node_rot: [f32; 4],
    center_idx: usize,
    base_idx: usize,
    overlay_idx: usize,
}

impl Default for SpinnerState {
    fn default() -> SpinnerState {
        SpinnerState {
            center_timer: 0.0,
            node_rot: [0.0; 4],
            center_idx: 0,
            base_idx: 0,
            overlay_idx: 0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<SpinnerState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SpinnerState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, SpinnerState::default) as *mut SpinnerState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    // 8x8 hitbox centered on the position: used only for the `collision.check`
    // solid probes in draw (corner/expand tests mirror `DustGraphic`).
    entity.hitbox.set(8.0, 8.0, -4.0, -4.0);
    entity.depth.set(-50);
    with_state(id, |st| {
        // Deterministic per-entity picks standing in for `Calc.Random`.
        st.center_idx = (id as usize) % CENTER_FRAMES.len();
        st.base_idx = (id as usize) % BASE_FRAMES.len();
        st.overlay_idx = (id as usize + 1) % OVERLAY_FRAMES.len();
        for (i, rot) in st.node_rot.iter_mut().enumerate() {
            *rot = (id as f32 + i as f32) * 0.7;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.center_timer += dt * CENTER_SPIN;
        for rot in &mut st.node_rot {
            *rot += dt * NODE_SPIN;
        }

        let entity = Entity::new(id);
        let p = entity.position.get();
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let px = pp.x + pox;
            let py = pp.y + poy;
            let overlap = px < p.x + KILL_W * 0.5
                && px + pw > p.x - KILL_W * 0.5
                && py < p.y + KILL_H * 0.5
                && py + ph > p.y - KILL_H * 0.5;
            if overlap {
                die();
            }
            break;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();

        let center_rot_deg = st.center_timer.to_degrees();
        draw_image(
            CENTER_FRAMES[st.center_idx],
            p.x,
            p.y,
            center_rot_deg,
            1.0,
            1.0,
        );

        for (i, (cx, cy)) in CORNERS.iter().enumerate() {
            // Skip corners occupied by solid tiles (`SolidCheck`).
            if entity.collision.check(cx * 4.0, cy * 4.0) {
                continue;
            }
            // `autoExpandDust`: stretch the node 5x along an axis when
            // another spinner or wall sits 16px out on that side.
            let mut vx = 1.0;
            let mut vy = 1.0;
            if entity.collision.check(cx * 16.0, cy * 4.0) {
                vx = 5.0;
            }
            if entity.collision.check(cx * 4.0, cy * 16.0) {
                vy = 5.0;
            }
            let n = (cx * cx + cy * cy).sqrt();
            let ax = cx / n * vx;
            let ay = cy / n * vy;

            let rot = st.node_rot[i].to_degrees();
            draw_image(BASE_FRAMES[st.base_idx], p.x + ax, p.y + ay, rot, 1.0, 1.0);
            draw_image(
                OVERLAY_FRAMES[st.overlay_idx],
                p.x + ax,
                p.y + ay,
                -rot,
                1.0,
                1.0,
            );
        }
    });
}
