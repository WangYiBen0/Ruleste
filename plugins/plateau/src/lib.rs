#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `plateau` entity plugin.
//!
//! The weathered rock slabs on the Reflection bridges. In the original these
//! are *solid* (`Solid`, 104×4, collider shifted 8 right so the standable run
//! is 96 wide), with `SurfaceSoundIndex = 23`. Level styling
//! (`Plateau.Level`) only picks the palette; the base stone stays.

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("plateau");
ruleste_plugin_api::ruleste_entity_types!("plateau");
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
    // Collider.Left += 8f on a 104-wide hitbox → standable run is 96 wide.
    entity.hitbox.set(96.0, 4.0, 8.0, 0.0);
    entity.collision.solid(true);
    entity.depth.set(0);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    // Original plateau stone slab, drawn from the gameplay atlas.
    draw_image("scenery/fallplateau", p.x + 52.0, p.y + 2.0, 0.0, 1.0, 1.0);
}
