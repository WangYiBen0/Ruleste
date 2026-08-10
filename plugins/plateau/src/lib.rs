#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `plateau` entity plugin.
//!
//! The weathered rock slabs on the Reflection bridges. In the original these
//! are background fixtures with no collision; the same is true here — the
//! plugin draws a mossy cap over a grey base. Level styling (`Plateau.Level`)
//! selects the palette; chapter 9 looks differently, but the base stone stays.

use ruleste_plugin_api::host::draw_rect;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("plateau");
ruleste_plugin_api::ruleste_entity_types!("plateau");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const STONE: Color = Color {
    r: 0x50,
    g: 0x56,
    b: 0x60,
    a: 0xff,
};
const MOSS: Color = Color {
    r: 0x6a,
    g: 0x78,
    b: 0x5a,
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
    entity.depth.set(300);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    draw_rect(p.x - 24.0, p.y - 9.0, 48.0, 9.0, STONE);
    draw_rect(p.x - 30.0, p.y - 11.0, 60.0, 3.0, MOSS);
}
