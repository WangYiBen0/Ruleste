#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-waterfall` — `waterfall` (universal).
//!
//! A falling sheet of water. Rendered as a translucent region with vertical
//! streaks; the original's splash/buoyancy is host-side, so this is visual.

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("waterfall");
ruleste_plugins_api::ruleste_entity_types!("waterfall");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const WATER: Color = Color { r: 0x4a, g: 0x88, b: 0xc8, a: 0x55 };
const STREAK: Color = Color { r: 0xbf, g: 0xe0, b: 0xff, a: 0x55 };

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 64.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.depth.set(-100);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let (w, h, _, _) = Entity::new(id).hitbox.get();
    draw_rect(p.x, p.y, w, h, WATER);
    let mut x = p.x + 3.0;
    while x < p.x + w - 1.0 {
        draw_rect(x, p.y, 1.0, h, STREAK);
        x += 5.0;
    }
}
