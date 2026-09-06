ruleste_plugins_api::ruleste_meta!("templeGate");
ruleste_plugins_api::ruleste_entity_types!("templeGate");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{drain_events, draw_rect};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

const GATE: Color = Color {
    r: 0xaa,
    g: 0x44,
    b: 0x44,
    a: 0xff,
};
const GATE_OPEN: Color = Color {
    r: 0x55,
    g: 0x55,
    b: 0x55,
    a: 0x55,
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
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    OPEN.with(|s| s.borrow_mut().insert(id, false));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let e = Entity::new(id);
    let opened = drain_events().iter().any(|(_, t, _)| *t == event::SWITCH);
    let mut open = OPEN.with(|s| s.borrow().get(&id).copied().unwrap_or(false));
    if opened {
        open = true;
    }
    e.collision.solid(!open);
    OPEN.with(|s| s.borrow_mut().insert(id, open));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();
    let open = OPEN.with(|s| s.borrow().get(&id).copied().unwrap_or(false));
    draw_rect(p.x, p.y, w, h, if open { GATE_OPEN } else { GATE });
}
