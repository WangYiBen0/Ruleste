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

// Per-type renderers. The host sets `entity.sprite.sprite` to the entity type at
// spawn, so the `ruleste_entity_*` FFI below dispatch by `sprite.bank()` to the
// matching module's init/update/draw. Types without a dedicated module (currently
// `dreamMirror`) fall back to a plain coloured box.
mod bird;
mod bonfire;
mod cliffflag;
mod cobweb;
mod debris;
mod flutterbird;
mod hanginglamp;
mod lamp;
mod lightbeam;
mod resort_lantern;
mod soundsource;
mod summitbackground;
mod torch;
mod towerviewer;
mod wire;

#[derive(Clone)]
struct State {
    w: f32,
    h: f32,
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
}

fn default_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
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

fn default_draw(id: EntityId) {
    let st = STATES.with(|s| s.borrow().get(&id).cloned());
    let (w, h) = match st {
        Some(st) => (st.w, st.h),
        None => return,
    };
    let p = Entity::new(id).position.get();
    draw_rect(p.x, p.y, w, h, color_for("decoration"));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    match Entity::new(id).sprite.bank().as_str() {
        "SummitBackgroundManager" => summitbackground::init(id, data, len),
        "bird" => bird::init(id, data, len),
        "bonfire" => bonfire::init(id, data, len),
        "cliffflag" => cliffflag::init(id, data, len),
        "cobweb" => cobweb::init(id, data, len),
        "floatingDebris" | "foregroundDebris" => debris::init(id, data, len),
        "flutterbird" => flutterbird::init(id, data, len),
        "hanginglamp" => hanginglamp::init(id, data, len),
        "lamp" => lamp::init(id, data, len),
        "lightbeam" => lightbeam::init(id, data, len),
        "resortLantern" => resort_lantern::init(id, data, len),
        "soundSource" => soundsource::init(id, data, len),
        "torch" => torch::init(id, data, len),
        "towerviewer" => towerviewer::init(id, data, len),
        "wire" => wire::init(id, data, len),
        // dreamMirror (and anything unrecognised) draws a plain coloured box.
        _ => default_init(id, data, len),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    match Entity::new(id).sprite.bank().as_str() {
        "SummitBackgroundManager" => summitbackground::update(id, dt),
        "bird" => bird::update(id, dt),
        "bonfire" => bonfire::update(id, dt),
        "cliffflag" => cliffflag::update(id, dt),
        "cobweb" => cobweb::update(id, dt),
        "floatingDebris" | "foregroundDebris" => debris::update(id, dt),
        "flutterbird" => flutterbird::update(id, dt),
        "hanginglamp" => hanginglamp::update(id, dt),
        "lamp" => lamp::update(id, dt),
        "lightbeam" => lightbeam::update(id, dt),
        "resortLantern" => resort_lantern::update(id, dt),
        "soundSource" => soundsource::update(id, dt),
        "torch" => torch::update(id, dt),
        "towerviewer" => towerviewer::update(id, dt),
        "wire" => wire::update(id, dt),
        _ => {}
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    match Entity::new(id).sprite.bank().as_str() {
        "SummitBackgroundManager" => summitbackground::draw(id),
        "bird" => bird::draw(id),
        "bonfire" => bonfire::draw(id),
        "cliffflag" => cliffflag::draw(id),
        "cobweb" => cobweb::draw(id),
        "floatingDebris" | "foregroundDebris" => debris::draw(id),
        "flutterbird" => flutterbird::draw(id),
        "hanginglamp" => hanginglamp::draw(id),
        "lamp" => lamp::draw(id),
        "lightbeam" => lightbeam::draw(id),
        "resortLantern" => resort_lantern::draw(id),
        "soundSource" => soundsource::draw(id),
        "torch" => torch::draw(id),
        "towerviewer" => towerviewer::draw(id),
        "wire" => wire::draw(id),
        _ => default_draw(id),
    }
}

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
