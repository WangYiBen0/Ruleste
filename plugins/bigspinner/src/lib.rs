#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `bigSpinner` entity plugin.
//!
//! The large variant of the dust spinner (`DangerBigSpinner` in the original
//! reflection rooms): a `radius=16` crystal ring. The sprite art is not in
//! the loaded atlas, so the ring is drawn procedurally as a slow-spinning
//! octagon of arcs; the lethal collider is a ~28px AABB covering the ring.

use ruleste_plugin_api::host::{self, die, draw_line, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("bigspinner");
ruleste_plugin_api::ruleste_entity_types!("bigSpinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const RADIUS: f32 = 16.0;
/// Covering AABB for the `Circle(16)` collider.
const KILL_W: f32 = 30.0;
const KILL_H: f32 = 30.0;

const SEGMENTS: usize = 8;

#[derive(Debug, Default)]
struct BigSpinnerState {
    angle: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BigSpinnerState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BigSpinnerState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BigSpinnerState::default) as *mut BigSpinnerState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn ring_point(cx: f32, cy: f32, radius: f32, angle: f32) -> (f32, f32) {
    (cx + angle.cos() * radius, cy + angle.sin() * radius)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity
        .hitbox
        .set(KILL_W, KILL_H, -KILL_W * 0.5, -KILL_H * 0.5);
    entity.depth.set(-50);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.angle += dt * 0.8;
        let p = Entity::new(id).position.get();
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + KILL_W * 0.5
                && pp.x + pox + pw > p.x - KILL_W * 0.5
                && pp.y + poy < p.y + KILL_H * 0.5
                && pp.y + poy + ph > p.y - KILL_H * 0.5;
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
        let p = Entity::new(id).position.get();
        let c = Color::new(0xb0, 0x84, 0xe8, 0xf0);
        let inner = RADIUS * 0.72;
        for i in 0..SEGMENTS {
            let a0 = st.angle + i as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            let a1 = st.angle + (i + 1) as f32 * std::f32::consts::TAU / SEGMENTS as f32;
            let (x0, y0) = ring_point(p.x, p.y, RADIUS, a0);
            let (x1, y1) = ring_point(p.x, p.y, RADIUS, a1);
            draw_line(x0, y0, x1, y1, c);
            let (ix0, iy0) = ring_point(p.x, p.y, inner, a0);
            draw_line(ix0, iy0, x0, y0, c);
        }
    });
}
