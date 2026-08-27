#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `bridge` entity plugin.
//!
//! A static wooden bridge: a solid platform the player can walk across to span
//! a gap. The platform size comes from the spawn's `width`/`height` attributes
//! (defaulting to a 32×8 plank), and it is a full solid so the player is
//! supported from above and blocked from below.

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("bridge");
ruleste_plugins_api::ruleste_entity_types!("bridge");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const PLANK: Color = Color {
    r: 0x8b,
    g: 0x5a,
    b: 0x2b,
    a: 0xff,
};
const PLANK_DARK: Color = Color {
    r: 0x5e,
    g: 0x3b,
    b: 0x1c,
    a: 0xff,
};

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let w = spawn.get_float("width", 32.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(2.0);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.collision.solid(true);
    entity.depth.set(200);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let (w, h, _, _) = Entity::new(id).hitbox.get();
    draw_rect(p.x, p.y, w, h, PLANK_DARK);
    // Draw plank seams every 8px so the slab reads as a bridge.
    let mut x = p.x + 8.0;
    while x < p.x + w - 1.0 {
        draw_rect(x - 1.0, p.y, 2.0, h, PLANK);
        x += 8.0;
    }
}
