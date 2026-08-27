#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{draw_rect, drain_events};
use ruleste_plugins_api::event;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("ridgeGate");
ruleste_plugins_api::ruleste_entity_types!("ridgeGate");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SHUT: Color = Color { r: 0xaa, g: 0x44, b: 0x44, a: 0xff };
const OPEN: Color = Color { r: 0x55, g: 0x55, b: 0x55, a: 0x55 };

#[derive(Copy, Clone)]
struct State {
    w: f32,
    h: f32,
    open: bool,
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    STATES.with(|s| {
        s.borrow_mut().insert(id, State { w, h, open: false });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let e = Entity::new(id);
    let opened = drain_events().iter().any(|(_src, t, _)| *t == event::SWITCH);
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).copied()) {
        Some(st) => st,
        None => return,
    };
    if opened {
        st.open = true;
    }
    e.collision.solid(!st.open);
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let st = STATES.with(|s| s.borrow().get(&id).copied());
    let (w, h, open) = match st {
        Some(st) => (st.w, st.h, st.open),
        None => (8.0, 8.0, false),
    };
    draw_rect(p.x, p.y, w, h, if open { OPEN } else { SHUT });
}
