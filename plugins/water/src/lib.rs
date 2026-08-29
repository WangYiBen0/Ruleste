#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-water` — `water` (universal).
//!
//! A body of water. The original applies swimming buoyancy to the player, which
//! requires host/player cooperation not yet exposed to plugins, so this renders
//! the translucent region only (gameplay buoyancy is handled by the host).

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("water");
ruleste_plugins_api::ruleste_entity_types!("water");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const WATER: Color = Color {
    r: 0x2a,
    g: 0x6c,
    b: 0xb0,
    a: 0x55,
};

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 32.0).max(4.0);
    let h = spawn.get_float("height", 32.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.depth.set(-100);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let (w, h, _, _) = Entity::new(id).hitbox.get();
    draw_rect(p.x, p.y, w, h, WATER);
}
