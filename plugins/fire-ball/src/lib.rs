#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{EV_SPRING_BOUNCE, die, draw_rect, emit, entities_by_type, remove};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("fireBall");
ruleste_plugins_api::ruleste_entity_types!("fireBall");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const FLAME: Color = Color {
    r: 0xff,
    g: 0x66,
    b: 0x22,
    a: 0xff,
};

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
    e.hitbox.set(w, h, -2.0, -2.0);
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
        let speed = 50.0;
        if dist <= speed * dt {
            e.position.set_xy(dest.0, dest.1);
            st.idx = (st.idx + 1) % st.nodes.len();
        } else {
            e.position
                .set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
        }
    }
    for pid in entities_by_type("player") {
        let pe = Entity::new(pid);
        let pp = pe.position.get();
        let (pw, ph, _, _) = pe.hitbox.get();
        let overlap = p.x - 2.0 < pp.x + pw
            && p.x + st.w + 2.0 > pp.x
            && p.y - 2.0 < pp.y + ph
            && p.y + st.h + 2.0 > pp.y;
        if !overlap {
            continue;
        }
        // `OnBounce`: a player landing on top of the fireball (feet in its
        // upper half) stomps it — the fireball is destroyed and the player is
        // bounced upward (best-effort `EV_SPRING_BOUNCE`; consumed by the
        // player plugin once it handles spring bounces).
        let stomp = pp.y < p.y && pp.y + ph <= p.y + st.h * 0.5;
        if stomp {
            let mut payload = [0u8; 13];
            payload[0..4].copy_from_slice(&pid.to_le_bytes());
            payload[4] = 0;
            payload[5..9].copy_from_slice(&p.x.to_le_bytes());
            payload[9..13].copy_from_slice(&p.y.to_le_bytes());
            emit(pid, EV_SPRING_BOUNCE, &payload);
            remove(id);
            return;
        }
        die();
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
    let st = STATES.with(|s| s.borrow().get(&id).cloned());
    let (w, h) = match st {
        Some(st) => (st.w, st.h),
        None => (8.0, 8.0),
    };
    draw_rect(p.x - 2.0, p.y - 2.0, w + 4.0, h + 4.0, FLAME);
}
