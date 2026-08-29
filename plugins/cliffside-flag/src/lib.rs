#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("cliffside_flag");
ruleste_plugins_api::ruleste_entity_types!("cliffside_flag");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const FLAG: Color = Color {
    r: 0xff,
    g: 0xff,
    b: 0xff,
    a: 0xff,
};
const POLE: Color = Color {
    r: 0x99,
    g: 0x99,
    b: 0x99,
    a: 0xff,
};

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 16.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(false);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();
    draw_rect(p.x, p.y, w, h, FLAG);
    draw_rect(p.x + w * 0.5 - 1.0, p.y, 2.0, h, POLE);
}
