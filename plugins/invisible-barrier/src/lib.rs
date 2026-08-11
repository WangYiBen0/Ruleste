#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `invisibleBarrier` entity plugin.
//!
//! An invisible wall (`Solid`) that keeps Madeline inside the hub rooms. The
//! host does not expose per-entity solid collision for arbitrary rectangles
//! (only tiled solids and dynamic platforms), so the barrier cannot push the
//! player yet: the plugin claims the entity slot so the room builds, and the
//! collision is a host-physics TODO. No visuals, as the name promises.

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("invisible-barrier");
ruleste_plugin_api::ruleste_entity_types!("invisibleBarrier");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    // Occupies its full pixel footprint so future solid-collision work can
    // query the footprint directly.
    entity.hitbox.set(
        spawn.get_float("width", 8.0),
        spawn.get_float("height", 8.0),
        0.0,
        0.0,
    );
    entity.depth.set(-1_000_000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
