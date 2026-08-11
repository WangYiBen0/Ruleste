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

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("heart-gem-door");
ruleste_plugin_api::ruleste_entity_types!("heartGemDoor");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const DOOR_FRAMES: [&str; 4] = [
    "objects/heartdoor/edge",
    "objects/heartdoor/top",
    "objects/heartdoor/icon00",
    "objects/heartdoor/icon01",
];

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
    entity.depth.set(0);
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
        // Frame edge / top / heart medallion from the gameplay atlas.
        draw_image(
            DOOR_FRAMES[0],
            p.x + ox + w * 0.5,
            p.y + oy + 2.0,
            0.0,
            1.0,
            1.0,
        );
        draw_image(
            DOOR_FRAMES[1],
            p.x + ox + w * 0.5,
            p.y + oy + h * 0.5,
            0.0,
            1.0,
            1.0,
        );
        draw_image(
            DOOR_FRAMES[2],
            p.x + ox + w * 0.5,
            p.y + oy + h * 0.7,
            0.0,
            1.0,
            1.0,
        );
        let _ = st.requires;
    });
}
