#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `redBlocks` / `yellowBlocks` / `greenBlocks` entity plugin.
//!
//! These are the Celestial Resort clutter blocks (`ClutterBlockBase.cs`):
//! standable solid platforms (marked via the host riding mechanism) rendered
//! as a dark translucent rectangle at depth 8999. The packed `ClutterBlock`
//! debris on top is not generated yet; the block simply appears as its base
//! silhouette.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("clutter");
ruleste_plugin_api::ruleste_entity_types!("redBlocks", "yellowBlocks", "greenBlocks");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `enabledColor` from the original: black at 70% alpha.
const ENABLED: Color = Color::new(0, 0, 0, 178);

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
    entity.depth.set(8999);
    entity.collision.solid(true);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let (w, h, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    host::draw_rect(p.x + ox, p.y + oy, w, h, ENABLED);
}
