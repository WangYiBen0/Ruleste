#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-color-switch` — `colorSwitch` (universal).
//!
//! A colored button that, in the original, toggles the solidity of the room's
//! matching-color solid tiles (a tile-grid operation the host performs). This
//! clone renders the button; the tile toggle is the host's responsibility, so
//! the plugin is a visual marker here. Mirrors `ColorSwitch` in
//! `references/source/Celeste/Celeste/ColorSwitch.cs` (visual portion).

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("color-switch");
ruleste_plugins_api::ruleste_entity_types!("colorSwitch");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const RING: Color = Color { r: 0xff, g: 0xff, b: 0xff, a: 0xff };

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
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    draw_rect(p.x - 8.0, p.y - 8.0, 16.0, 16.0, RING);
    draw_rect(p.x - 3.0, p.y - 3.0, 6.0, 6.0, RING);
}
