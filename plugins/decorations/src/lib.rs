//! `ruleste-plugin-decorations` — the catch-all rendering sink for every
//! entity type listed in `marker.toml` that has no other owner.
//!
//! The host engine reads `marker.toml` at compile time to decide which spawns
//! land in `room.decorations` instead of `room.entities`. This plugin then
//! registers every orphan decoration type so the engine can find a renderer
//! for them, and dispatches the draw call by `_entity_type`. None of these
//! types have any gameplay side effect: they are sprite-only.
//!
//! The marker file is the single source of truth. Adding a new decoration
//! type means: add the name to `marker.toml`, rebuild the host (it
//! re-`include_str!`s the file), and add it to this macro.

#![allow(clippy::not_unsafe_ptr_arg_deref)]
use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};
use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("decorations");
// These 17 types are in marker.toml but have no other plugin owner.
// If a new gameplay plugin later claims one of these, remove it from here.
ruleste_plugins_api::ruleste_entity_types!(
    "SummitBackgroundManager",
    "bird",
    "bonfire",
    "cliffflag",
    "cobweb",
    "dreamMirror",
    "floatingDebris",
    "flutterbird",
    "foregroundDebris",
    "hanginglamp",
    "lamp",
    "lightbeam",
    "resortLantern",
    "soundSource",
    "torch",
    "towerviewer",
    "wire"
);
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[derive(Clone)]
struct State {
    w: f32,
    h: f32,
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(2.0);
    let h = spawn.get_float("height", 16.0).max(2.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(false);
    STATES.with(|s| {
        s.borrow_mut().insert(id, State { w, h });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

fn color_for(t: &str) -> Color {
    let h: u32 = t.bytes().fold(0x8110_C8A5u32, |acc, b| {
        acc.wrapping_mul(16777619) ^ b as u32
    });
    Color {
        r: ((h >> 16) & 0xff) as u8,
        g: ((h >> 8) & 0xff) as u8,
        b: (h & 0xff) as u8,
        a: 0xff,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = STATES.with(|s| s.borrow().get(&id).cloned());
    let (w, h) = match st {
        Some(st) => (st.w, st.h),
        None => return,
    };
    let p = Entity::new(id).position.get();
    draw_rect(p.x, p.y, w, h, color_for("decoration"));
}
