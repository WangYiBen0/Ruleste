#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `SummitBackgroundManager` entity plugin.
//!
//! In the original this manages the layered summit background (the rotating
//! flags, clouds, and the snow wind that layers across chapter 7). The host's
//! backdrop system does not expose those layers yet, so the plugin only owns
//! the entity slot and its `index`/`intro_launch` attributes; there is
//! nothing to update or draw until a sky system lands.

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("summitbackground");
ruleste_plugin_api::ruleste_entity_types!("SummitBackgroundManager");
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
    entity.depth.set(-1_000_000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
