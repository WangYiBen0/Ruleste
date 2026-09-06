#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::draw_image;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    // Real fixed bridge sprite (scenery/bridge_fixed, 208x55).
    draw_image("scenery/bridge_fixed", p.x, p.y, 0.0, 1.0, 1.0);
}
