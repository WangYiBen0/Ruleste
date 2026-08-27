#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `hahaha` entity plugin — placeholder.
//!
//! The converted content references a `hahaha` entity but ships no sprite or
//! behavioral metadata for it, so for now it renders a simple marker and
//! carries no gameplay behavior. Swap in real logic once the entity's semantics
//! are known (collectible, hazard, trigger, …).

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("hahaha");
ruleste_plugins_api::ruleste_entity_types!("hahaha");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const MARK: Color = Color {
    r: 0xff,
    g: 0x40,
    b: 0x40,
    a: 0xff,
};

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    // No-op hitbox; this placeholder does not collide.
    entity.hitbox.set(8.0, 8.0, 0.0, 0.0);
    entity.depth.set(500);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    draw_rect(p.x, p.y, 8.0, 8.0, MARK);
}
