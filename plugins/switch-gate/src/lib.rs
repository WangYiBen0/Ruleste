#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-switch-gate` — `switchGate` from the Forsaken City maps.
//!
//! A solid block that, when a `touchSwitch` somewhere in the room fires the
//! `SWITCH` event (see `references/source/Celeste/Celeste/SwitchGate.cs`),
//! slides to its configured `node` and opens (becomes non-solid) so the player
//! can pass through where it stood. Listens on the event bus, so it must live in
//! its own plugin crate (the host keeps one event cursor per crate).

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{drain_events, draw_rect};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("switch-gate");
ruleste_plugins_api::ruleste_entity_types!("switchGate");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const GATE: Color = Color {
    r: 0x5f,
    g: 0xcd,
    b: 0xe4,
    a: 0xcc,
};
const GATE_OPEN: Color = Color {
    r: 0x5f,
    g: 0xcd,
    b: 0xe4,
    a: 0x30,
};

#[derive(Clone, Copy)]
struct State {
    w: f32,
    h: f32,
    open: bool,
    t: f32,
    start: Vec2,
    node: Vec2,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn spawn_type(data: *const u8, len: u32) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    spawn_data(bytes).get_str("_entity_type", "")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    if spawn_type(data, len) != "switchGate" {
        return;
    }
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 32.0).max(4.0);
    let h = spawn.get_float("height", 32.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    e.depth.set(200);
    let node = spawn.get_node(0).unwrap_or(Vec2::new(x, y));
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                w,
                h,
                open: false,
                t: 0.0,
                start: Vec2::new(x, y),
                node,
            },
        )
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    if st.open {
        if st.t < 1.0 {
            st.t = (st.t + dt / 1.8).min(1.0);
            let nx = st.start.x + (st.node.x - st.start.x) * st.t;
            let ny = st.start.y + (st.node.y - st.start.y) * st.t;
            e.position.set_xy(nx, ny);
            if st.t >= 1.0 {
                e.collision.solid(false);
            }
        }
    } else {
        for (_, kind, _) in drain_events() {
            if kind == event::SWITCH {
                st.open = true;
                break;
            }
        }
    }
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow_mut().get(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let p = Entity::new(id).position.get();
    let c = if st.open { GATE_OPEN } else { GATE };
    draw_rect(p.x, p.y, st.w, st.h, c);
}
