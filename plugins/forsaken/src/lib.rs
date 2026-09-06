#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-forsaken` — Forsaken City (chapter 1) specific entities.
//!
//! * `memorialTextController` -> invisible cutscene trigger (no gameplay).

use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("forsaken");
ruleste_plugins_api::ruleste_entity_types!("memorialTextController");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(200);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {
    // Invisible trigger — no rendering.
}
