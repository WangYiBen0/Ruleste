#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-dream-mirror` — `dreammirror` (universal, dream mechanic).
//!
//! The reflective glass the player steps into for dream-dash reflection
//! sequences. The original renders a live reflection via a render target, which
//! this clone does not have, so it is drawn as a translucent framed glass panel
//! with no collision. Mirrors `DreamMirror` in
//! `references/source/Celeste/Celeste/DreamMirror.cs` (visual portions only).

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("dream-mirror");
ruleste_plugins_api::ruleste_entity_types!("dreammirror");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const FRAME: Color = Color {
    r: 0xb0,
    g: 0xc4,
    b: 0xde,
    a: 0xff,
};
const GLASS: Color = Color {
    r: 0x6a,
    g: 0x8c,
    b: 0xb0,
    a: 0x55,
};

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(8.0);
    let h = spawn.get_float("height", 24.0).max(8.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.depth.set(9500);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let (w, h, _, _) = Entity::new(id).hitbox.get();
    draw_rect(p.x, p.y, w, h, GLASS);
    draw_rect(p.x, p.y, w, 2.0, FRAME);
    draw_rect(p.x, p.y + h - 2.0, w, 2.0, FRAME);
    draw_rect(p.x, p.y, 2.0, h, FRAME);
    draw_rect(p.x + w - 2.0, p.y, 2.0, h, FRAME);
}
