#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `introCar` entity plugin.
//!
//! The crashed car Madeline and Theo sit on during the prologue (and a vanity
//! prop in the chapter-9 hub). The cutscenes and the mount behavior are host
//! scripting features; this plugin just draws the car as a small procedural
//! silhouette with a subtle idle sway.

use ruleste_plugin_api::host::draw_rect;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("introcar");
ruleste_plugin_api::ruleste_entity_types!("introCar");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const BODY: Color = Color {
    r: 0x2c,
    g: 0x34,
    b: 0x44,
    a: 0xff,
};
const CABIN: Color = Color {
    r: 0x6c,
    g: 0x7c,
    b: 0x9c,
    a: 0xff,
};
const WHEEL: Color = Color {
    r: 0x16,
    g: 0x1a,
    b: 0x22,
    a: 0xff,
};

#[derive(Debug, Default)]
struct CarState {
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CarState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CarState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CarState::default) as *mut CarState;
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
    entity.depth.set(1000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Car body +
        let sway = (st.timer * 0.8).sin() * 0.5;
        draw_rect(p.x - 24.0, p.y - 10.0 + sway, 48.0, 10.0, BODY);
        draw_rect(p.x - 14.0, p.y - 16.0 + sway, 28.0, 6.0, CABIN);
        draw_rect(p.x - 26.0, p.y - 2.0, 12.0, 6.0, WHEEL);
        draw_rect(p.x + 14.0, p.y - 2.0, 12.0, 6.0, WHEEL);
    });
}
