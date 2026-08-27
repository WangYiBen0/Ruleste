#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::{draw_rect, entities_by_type, die};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("core");
ruleste_plugins_api::ruleste_entity_types!(
    "coreMessage",
    "coreModeToggle",
    "risingLava",
    "sandwichLava"
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const LAVA: Color = Color { r: 0xff, g: 0x66, b: 0x22, a: 0xcc };
const MSG: Color = Color { r: 0xaa, g: 0xaa, b: 0xaa, a: 0xff };
const TOGGLE: Color = Color { r: 0x44, g: 0x88, b: 0xcc, a: 0xff };

#[derive(Copy, Clone, PartialEq, Eq)]
enum Kind {
    CoreMessage,
    CoreModeToggle,
    RisingLava,
    SandwichLava,
}

fn kind_for(t: &str) -> Option<Kind> {
    Some(match t {
        "coreMessage" => Kind::CoreMessage,
        "coreModeToggle" => Kind::CoreModeToggle,
        "risingLava" => Kind::RisingLava,
        "sandwichLava" => Kind::SandwichLava,
        _ => return None,
    })
}

#[derive(Copy, Clone)]
struct State {
    kind: Kind,
    w: f32,
    h: f32,
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
}

fn spawn_type(data: *const u8, len: u32) -> String {
    spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) }).get_str("_entity_type", "")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let Some(kind) = kind_for(&spawn_type(data, len)) else {
        return;
    };
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    STATES.with(|s| {
        s.borrow_mut().insert(id, State { kind, w, h });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let st = match STATES.with(|s| s.borrow().get(&id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let mut p = e.position.get();
    if st.kind == Kind::RisingLava {
        p.y -= 4.0 * dt;
        e.position.set_xy(p.x, p.y);
    }
    if st.kind == Kind::RisingLava || st.kind == Kind::SandwichLava {
        for pid in entities_by_type("player") {
            let pp = Entity::new(pid).position.get();
            let (pw, ph, _, _) = Entity::new(pid).hitbox.get();
            if p.x < pp.x + pw && p.x + st.w > pp.x && p.y < pp.y + ph && p.y + st.h > pp.y {
                die();
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow().get(&id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let p = e.position.get();
    let c = match st.kind {
        Kind::RisingLava | Kind::SandwichLava => LAVA,
        Kind::CoreMessage => MSG,
        Kind::CoreModeToggle => TOGGLE,
    };
    draw_rect(p.x, p.y, st.w, st.h, c);
}
