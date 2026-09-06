ruleste_plugins_api::ruleste_meta!("trapdoor");
ruleste_plugins_api::ruleste_entity_types!("trapdoor");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

const TRAP: Color = Color {
    r: 0x6a,
    g: 0x5a,
    b: 0x4a,
    a: 0xff,
};
const TRAP_OPEN: Color = Color {
    r: 0x6a,
    g: 0x5a,
    b: 0x4a,
    a: 0x20,
};

thread_local! {
    static OPEN: RefCell<std::collections::HashMap<EntityId, bool>> =
        RefCell::new(std::collections::HashMap::new());
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    OPEN.with(|s| s.borrow_mut().insert(id, false));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();
    let open = OPEN.with(|s| s.borrow().get(&id).copied().unwrap_or(false));
    draw_rect(p.x, p.y, w, h, if open { TRAP_OPEN } else { TRAP });
}
