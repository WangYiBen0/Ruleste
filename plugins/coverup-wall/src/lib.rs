#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-coverup-wall` — `coverupWall` (universal).
//!
//! A static solid filler that hides a seam in the foreground tiles. Mirrors
//! `CoverupWall` in `references/source/Celeste/Celeste/CoverupWall.cs`.

use ruleste_plugins_api::host::draw_tile_box;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("coverup-wall");
ruleste_plugins_api::ruleste_entity_types!("coverupWall");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 8.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    e.depth.set(-13000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let (w, h, _, _) = Entity::new(id).hitbox.get();
    draw_tile_box('3', p.x, p.y, (w / 8.0) as u32, (h / 8.0) as u32);
}
