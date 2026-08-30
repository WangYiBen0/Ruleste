#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::draw_image;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("gondola");
ruleste_plugins_api::ruleste_entity_types!("gondola");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[derive(Clone)]
struct State {
    w: f32,
    h: f32,
    nodes: Vec<(f32, f32)>,
    idx: usize,
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
    e.collision.platform(true);
    let nodes: Vec<(f32, f32)> = spawn.nodes().iter().map(|n| (n.x, n.y)).collect();
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                w,
                h,
                nodes,
                idx: 0,
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).cloned()) {
        Some(st) => st,
        None => return,
    };
    if !st.nodes.is_empty() {
        let dest = st.nodes[st.idx];
        let dx = dest.0 - p.x;
        let dy = dest.1 - p.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let speed = 40.0;
        if dist <= speed * dt {
            e.position.set_xy(dest.0, dest.1);
            st.idx = (st.idx + 1) % st.nodes.len();
        } else {
            e.position
                .set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
        }
    }
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    // The real gondola cabin — drawn at native size over the platform hitbox.
    draw_image("objects/gondola/front", p.x, p.y, 0.0, 1.0, 1.0);
}
