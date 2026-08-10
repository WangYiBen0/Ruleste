#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `zipMover` entity plugin.
//!
//! Mirrors `ZipMover.cs`: a solid box that slides back and forth between its
//! spawn point and its node, carrying anything standing on it. The motion is
//! a sine ease (slow at both ends), each leg taking `LEG_TIME`. Movement goes
//! through `actor_move` so riders are carried and the block stops at solid
//! walls just like the original.

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("zipmover");
ruleste_plugin_api::ruleste_entity_types!("zipMover");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// One full traversal (there and back) in seconds, close to the original.
const CYCLE_TIME: f32 = 3.4;

#[derive(Debug, Default)]
struct MoverState {
    from: Vec2,
    to: Vec2,
    /// Progress in [0, 2): 0..1 = leg out, 1..2 = leg back.
    phase: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<MoverState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut MoverState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, MoverState::default) as *mut MoverState;
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
    let w = spawn.get_float("width", 16.0);
    let h = spawn.get_float("height", 16.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(500);
    entity.collision.platform(true);
    with_state(id, |st| {
        st.from = Vec2::new(x, y);
        st.to = spawn.get_node(0).unwrap_or(Vec2::new(x + 48.0, y));
    });
}

/// Target position for the current phase point, eased.
fn target(st: &MoverState) -> Vec2 {
    let t = st.phase.min(1.0);
    let ease = 0.5 - 0.5 * (t * std::f32::consts::PI).cos();
    let a = if st.phase < 1.0 { st.from } else { st.to };
    let b = if st.phase < 1.0 { st.to } else { st.from };
    Vec2::new(a.x + (b.x - a.x) * ease, a.y + (b.y - a.y) * ease)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.phase += dt / CYCLE_TIME;
        st.phase %= 2.0;
        let t = target(st);
        let entity = Entity::new(id);
        let cur = entity.position.get();
        // `actor_move` carries riders and stops at solid walls; the collision
        // flags are irrelevant while the mover simply sweeps along its track.
        let _on_move = entity.collision.actor_move(t.x - cur.x, t.y - cur.y);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    // Solid timber slab.
    ruleste_plugin_api::host::draw_rect(p.x + ox, p.y + oy, w, h, crate_color());
}

fn crate_color() -> ruleste_plugin_api::types::Color {
    ruleste_plugin_api::types::Color::new(0x58, 0x3c, 0x20, 0xff)
}
