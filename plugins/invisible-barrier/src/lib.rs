#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `invisibleBarrier` entity plugin.
//!
//! An invisible wall (`Solid`) that keeps Madeline inside the hub rooms.
//! Collides on all four sides via the host's solid-platform set; the player
//! cannot pass through but dashing against it stops cleanly.

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
    entity.hitbox.set(
        spawn.get_float("width", 8.0),
        spawn.get_float("height", 8.0),
        0.0,
        0.0,
    );
    entity.collision.solid(true);
    entity.depth.set(-1_000_000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
