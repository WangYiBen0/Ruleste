#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `dashBlock` entity plugin.
//!
//! The sealed resort stone (`DashBlock.cs`): a solid tile slab that only gives
//! way when Madeline dashes into it with enough momentum, then unlocks and
//! stays broken for the session. `permanent` blocks always shatter; the
//! `tiletype`/`canDash` attributes are recorded but the host has no tile
//! swap API, so breaking simply removes the slab (via `host_collect`, keeping
//! it out of respawns like the original).

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("dashblock");
ruleste_plugin_api::ruleste_entity_types!("dashBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Dash slam threshold: a normal jump/walk can't break the seal.
const HIT_SPEED: f32 = 240.0;

#[derive(Debug, Default)]
struct DashBlockState {
    /// Mapped box for drawing (the collision rect lives on the entity).
    w: f32,
    h: f32,
    moving: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DashBlockState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DashBlockState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DashBlockState::default) as *mut DashBlockState;
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
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(500);
    with_state(id, |st| {
        st.w = w;
        st.h = h;
        let _ = spawn.get_bool("permanent", true);
        let _ = spawn.get_str("tiletype", "b");
    });
}

fn hitbox_overlap(world_id: EntityId) -> (bool, Vec2) {
    let p = Entity::new(world_id).position.get();
    let (w, h, ox, oy) = Entity::new(world_id).hitbox.get();
    let mut hit = false;
    let mut vel = Vec2::ZERO;
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let overlap = pp.x + pox < p.x + ox + w
            && pp.x + pox + pw > p.x + ox
            && pp.y + poy < p.y + oy + h
            && pp.y + poy + ph > p.y + oy;
        if overlap {
            hit = true;
            vel = host::Speed::new(player_id).get();
            break;
        }
    }
    (hit, vel)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    with_state(id, |st| {
        if st.moving {
            return;
        }
        // Moving off-screen as a "shattering" preamble would need a real
        // particle system; unlock is also granted immediately here so the
        // collision rect opens up the moment the slab pops.
        let (hit, vel) = hitbox_overlap(id);
        if hit && (vel.x.abs() >= HIT_SPEED || vel.y.abs() >= HIT_SPEED) {
            st.moving = true;
            host::collect(id);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if st.moving {
            return;
        }
        let p = Entity::new(id).position.get();
        // Dark resort masonry with a thin swallow "wear" edge.
        ruleste_plugin_api::host::draw_rect(p.x, p.y, st.w, st.h, color_fill());
        ruleste_plugin_api::host::draw_rect(p.x, p.y, st.w, 2.0, color_edge());
    });
}

fn color_fill() -> ruleste_plugin_api::types::Color {
    ruleste_plugin_api::types::Color::new(0x32, 0x3a, 0x48, 0xff)
}

fn color_edge() -> ruleste_plugin_api::types::Color {
    ruleste_plugin_api::types::Color::new(0x9a, 0x78, 0x50, 0xff)
}
