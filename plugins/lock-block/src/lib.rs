#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-lock-block` — `lockBlock` (universal).
//!
//! A solid that opens (becomes non-solid) when a `key` somewhere in the room
//! emits the `KEY` event. Mirrors `LockBlock` in
//! `references/source/Celeste/Celeste/LockBlock.cs`. It only listens, so it lives
//! in its own crate.

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{drain_events, draw_image, draw_rect};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("lock-block");
ruleste_plugins_api::ruleste_entity_types!("lockBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const LOCK_OPEN: Color = Color {
    r: 0x9a,
    g: 0x6a,
    b: 0x3a,
    a: 0x30,
};

#[derive(Clone, Copy)]
struct State {
    w: f32,
    h: f32,
    open: bool,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
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
    e.depth.set(200);
    STATES.with(|s| s.borrow_mut().insert(id, State { w, h, open: false }));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    if !st.open {
        for (_, kind, _) in drain_events() {
            if kind == event::KEY {
                st.open = true;
                Entity::new(id).collision.solid(false);
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
    if !st.open {
        // Real lock door sprite (objects/door/lockdoor00, 32x32).
        draw_image("objects/door/lockdoor00", p.x, p.y, 0.0, 1.0, 1.0);
    } else {
        // Open: faded ghost.
        draw_rect(p.x, p.y, st.w, st.h, LOCK_OPEN);
    }
}
