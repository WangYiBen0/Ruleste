#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{draw_rect, entities_by_type, die};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("seeker");
ruleste_plugins_api::ruleste_entity_types!("seeker");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const BODY: Color = Color { r: 0x22, g: 0x22, b: 0x33, a: 0xff };
const EYE: Color = Color { r: 0xff, g: 0x55, b: 0x55, a: 0xff };

#[derive(Copy, Clone)]
struct State {
    base: (f32, f32),
    chasing: bool,
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
    e.hitbox.set(14.0, 14.0, -3.0, -3.0);
    STATES.with(|s| {
        s.borrow_mut().insert(id, State { base: (x, y), chasing: false });
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
    let players = entities_by_type("player");
    let mut target: Option<(f32, f32)> = None;
    for pid in players {
        let pp = Entity::new(pid).position.get();
        let dx = pp.x - p.x;
        let dy = pp.y - p.y;
        if (dx * dx + dy * dy).sqrt() < 120.0 {
            target = Some((pp.x, pp.y));
            break;
        }
    }
    match target {
        Some((tx, ty)) => {
            st.chasing = true;
            let dx = tx - p.x;
            let dy = ty - p.y;
            let dist = (dx * dx + dy * dy).sqrt();
            let speed = 55.0;
            if dist > 1.0 {
                e.position.set_xy(
                    p.x + dx / dist * speed * dt,
                    p.y + dy / dist * speed * dt,
                );
            }
            if dist < 10.0 {
                die();
            }
        }
        None => {
            st.chasing = false;
            let dx = st.base.0 - p.x;
            let dy = st.base.1 - p.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > 1.0 {
                let speed = 30.0;
                e.position.set_xy(
                    p.x + dx / dist * speed * dt,
                    p.y + dy / dist * speed * dt,
                );
            }
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
    let e = Entity::new(id);
    let p = e.position.get();
    let st = STATES.with(|s| s.borrow().get(&id).copied());
    let chasing = st.map(|s| s.chasing).unwrap_or(false);
    draw_rect(p.x - 7.0, p.y - 7.0, 14.0, 14.0, BODY);
    draw_rect(p.x - 3.0, p.y - 3.0, 6.0, 6.0, if chasing { EYE } else { BODY });
}
