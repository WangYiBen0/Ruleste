#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `heartGemDoor` entity plugin.
//!
//! The chapter-9 gate (`HeartGemDoor`): a tall stone wall that swings open
//! when the climber holds the required heart count. The host has no save/heart
//! inventory to query yet, so the door renders closed and inert for now —
//! the `requires` counter is remembered in state for when saves land.
//!
//! Note: the map's door bars are also baked into the binary's solid grid, so
//! making the door move before the inventory system exists would trap the
//! player; resting closed is the safe default.

use ruleste_plugin_api::host::draw_rect;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("heartgemdoor");
ruleste_plugin_api::ruleste_entity_types!("heartGemDoor");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const DOOR: Color = Color {
    r: 0x3e,
    g: 0x44,
    b: 0x52,
    a: 0xff,
};
const EDGE: Color = Color {
    r: 0x66,
    g: 0x70,
    b: 0x86,
    a: 0xff,
};
const HEART: Color = Color {
    r: 0xd0,
    g: 0x38,
    b: 0x60,
    a: 0xff,
};

#[derive(Debug, Default)]
struct DoorState {
    requires: u32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DoorState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DoorState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DoorState::default) as *mut DoorState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.hitbox.set(
        spawn.get_float("width", 88.0),
        spawn.get_float("height", 96.0),
        0.0,
        0.0,
    );
    entity.depth.set(500);
    with_state(id, |st| {
        st.requires = spawn.get_float("requires", 0.0) as u32;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let (w, h, ox, oy) = Entity::new(id).hitbox.get();
        // Door slab with a heart medallion at its center.
        draw_rect(p.x + ox, p.y + oy, w, h, DOOR);
        draw_rect(p.x + ox, p.y + oy, w, 3.0, EDGE);
        let hx = p.x + w * 0.5;
        let hy = p.y + oy + h * 0.6;
        draw_rect(hx - 6.0, hy, 12.0, 8.0, HEART);
        draw_rect(hx - 2.0, hy - 6.0, 4.0, 6.0, HEART);
        let _ = st.requires;
    });
}
