#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("moveBlock");
ruleste_plugins_api::ruleste_entity_types!("moveBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const BLOCK: Color = Color { r: 0x88, g: 0x88, b: 0x88, a: 0xff };

#[derive(Copy, Clone)]
struct State {
    w: f32,
    h: f32,
    base: (f32, f32),
    target: (f32, f32),
    to_node: bool,
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
    let target = spawn.get_node(0).map(|n| (n.x, n.y)).unwrap_or((x, y));
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                w,
                h,
                base: (x, y),
                target,
                to_node: true,
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).copied()) {
        Some(st) => st,
        None => return,
    };
    let speed = 60.0;
    let dest = if st.to_node { st.target } else { st.base };
    let dx = dest.0 - p.x;
    let dy = dest.1 - p.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= speed * dt {
        e.position.set_xy(dest.0, dest.1);
        st.to_node = !st.to_node;
    } else {
        e.position.set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
    }
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
    let (w, h) = match st {
        Some(st) => (st.w, st.h),
        None => (8.0, 8.0),
    };
    draw_rect(p.x, p.y, w, h, BLOCK);
}
